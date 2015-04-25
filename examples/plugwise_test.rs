//extern crate serial;
extern crate time;
extern crate crc16;
extern crate toml;

mod messages;
mod stub;

use std::io;
use std::io::prelude::*;
//use serial::prelude::*;
//use time::Duration;
use crc16::*;
use std::fs::File;
use std::env::home_dir;
use messages::*;

const HEADER: [u8; 4] = [5, 5, 3, 3];
const FOOTER: [u8; 2] = [13, 10];
const EOM: u8 = 10;
const CRC_SIZE: usize = 4;

enum ProtocolSnoop<'a> {
    Nothing,
    Debug(&'a mut Write),
    Raw(&'a mut Write),
    All(&'a mut Write)
}

struct Protocol<'a, R> {
    reader: io::BufReader<R>,
    snoop: ProtocolSnoop<'a>
}

impl<'a, R: Read + Write> Protocol<'a, R> {
    /// Wrap IO entity for Plugwise protocol handling
    fn new(port: R) -> Protocol<'a, R> {
        Protocol {
            reader: io::BufReader::with_capacity(1000, port),
            snoop: ProtocolSnoop::Nothing
        }
    }

    fn set_snoop(&mut self, snoop: ProtocolSnoop<'a>) {
        self.snoop = snoop;
    }

    /// Send payload
    fn send_message_raw(&mut self, payload: &[u8]) -> io::Result<()> {
        let crc = format!("{:04X}", State::<XMODEM>::calculate(payload)).into_bytes();

        try!(self.reader.get_mut().write(&HEADER));
        try!(self.reader.get_mut().write(payload));
        try!(self.reader.get_mut().write(&crc));
        try!(self.reader.get_mut().write(&FOOTER));

        match self.snoop {
            ProtocolSnoop::Raw(ref mut writer) |
            ProtocolSnoop::All(ref mut writer) => {
                try!(writer.write_fmt(format_args!("> ")));
                try!(writer.write(payload));
                try!(writer.write(&crc));
                try!(writer.write(&[b'\n']));
            },
            _ => {}
        }

        Ok(())
    }

    /// Wait until a Plugwise message has been received (and skip debugging stuff)
    fn receive_plugwise_message_raw(&mut self) -> io::Result<(Vec<u8>, usize, usize)> {
        loop {
            let mut buf = vec![];

            let _ = try!(self.reader.read_until(EOM, &mut buf));

            let header_pos = buf.windows(HEADER.len()).position(|x| *x==HEADER);

            if header_pos.is_some() {
                let header_pos = header_pos.unwrap(); // that would be a surprise when this panics

                let footer_pos = match buf.windows(FOOTER.len()).rposition(|x| *x==FOOTER){
                    None => return Err(io::Error::new(io::ErrorKind::Other,
                                                      "unable to locate footer in received message")),
                                                      Some(v) => v
                };

                match self.snoop {
                    ProtocolSnoop::Raw(ref mut writer) |
                    ProtocolSnoop::All(ref mut writer) => {
                        let (_, part) = buf.split_at(header_pos + HEADER.len());
                        let (part, _) = part.split_at(footer_pos - (header_pos + HEADER.len()));
                        try!(writer.write_fmt(format_args!("< ")));
                        try!(writer.write(part));
                        try!(writer.write(&[b'\n']));
                    },
                    _ => {}
                }

                return Ok((buf, header_pos, footer_pos))
            }
            else
            {
                match self.snoop {
                    ProtocolSnoop::All(ref mut writer) => {
                        let footer_pos = buf.windows(FOOTER.len()).rposition(|x| *x==FOOTER);

                        if let Some(pos) = footer_pos {
                            let (part, _) = buf.split_at(pos);
                            try!(writer.write_fmt(format_args!("< ")));
                            try!(writer.write(part));
                            try!(writer.write(&[b'\n']));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Wait until a complete and valid message has been received
    fn receive_message_raw(&mut self) -> io::Result<Vec<u8>> {
        let (buf, header_pos, footer_pos) = try!(self.receive_plugwise_message_raw());

        // chop off header, footer and CRC
        let payload = buf.iter().take(footer_pos - CRC_SIZE).skip(header_pos + HEADER.len());
        let crc = buf.iter().skip(footer_pos - CRC_SIZE).take(CRC_SIZE);
        let crc = crc.take(4).fold(0, |acc, &item| {
            acc << 4 | match item {
                b'0' => 0, b'1' => 1, b'2' => 2,  b'3' => 3,  b'4' => 4,  b'5' => 5,  b'6' => 6,  b'7' => 7,
                b'8' => 8, b'9' => 9, b'A' => 10, b'B' => 11, b'C' => 12, b'D' => 13, b'E' => 14, b'F' => 15,
                _ => 0
            }
        });

        // CRC check
        let mut state = State::<XMODEM>::new();
        for byte in payload {
            state.update(&[*byte]);
        }

        if crc != state.get() {
            return Err(io::Error::new(io::ErrorKind::Other, "CRC error"));
        }

        let payload = buf.iter().take(footer_pos - CRC_SIZE).skip(header_pos + HEADER.len());

        Ok(payload.cloned().collect())
    }

    /// Keep receiving messages until the given message identifier has been received
    fn expect_message(&mut self, expected_message_id: MessageId) -> io::Result<Message> {
        loop {
            let msg = try!(self.receive_message_raw());
            let msg = try!(Message::from_payload(&msg));

            match self.snoop {
                ProtocolSnoop::Debug(ref mut writer) => {
                    try!(writer.write_fmt(format_args!("< {:?}\n", msg)));
                },
                _ => {}
            }

            if msg.to_message_id() == expected_message_id {
                return Ok(msg)
            }
        }
    }

    fn wait_for_mac_ack(&mut self, expected_mac: u64) -> io::Result<()> {
        loop {
            let ack = try!(self.expect_message(MessageId::Ack));
            if let Message::Ack(_, ack) = ack {
                if let Some(ack_mac) = ack.mac {
                    if ack_mac == expected_mac {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Send message
    fn send_message(&mut self, message: Message) -> io::Result<()> {
        match self.snoop {
            ProtocolSnoop::Debug(ref mut writer) => {
                try!(writer.write_fmt(format_args!("> {:?}\n", message)));
            },
            _ => {}
        }
        let msg = try!(message.to_payload());
        try!(self.send_message_raw(&msg));
        Ok(())
    }

    /// Initialize the Plugwise USB stick
    fn initialize(&mut self) -> io::Result<ResInitialize> {
        try!(self.send_message(Message::ReqInitialize));

        let msg = try!(self.expect_message(MessageId::ResInitialize));

        match msg {
            Message::ResInitialize(_, res) => Ok(res),
            _ => Err(io::Error::new(io::ErrorKind::Other, "unexpected initialization response"))
        }
    }

    /// Get info from a circle
    fn get_info(&mut self, mac: u64) -> io::Result<ResInfo> {
        try!(self.send_message(Message::ReqInfo(ReqHeader{mac: mac})));

        let msg = try!(self.expect_message(MessageId::ResInfo));

        match msg {
            Message::ResInfo(_, res) => Ok(res),
            _ => Err(io::Error::new(io::ErrorKind::Other, "unexpected information response"))
        }
    }

    /// Switch a circle
    fn switch(&mut self, mac: u64, on: bool) -> io::Result<()> {
        try!(self.send_message(Message::ReqSwitch(ReqHeader{mac: mac}, ReqSwitch{on: on})));

        try!(self.wait_for_mac_ack(mac));

        Ok(())
    }

    /// Calibrate a circle
    fn calibrate(&mut self, mac: u64) -> io::Result<ResCalibration> {
        try!(self.send_message(Message::ReqCalibration(ReqHeader{mac: mac})));

        let msg = try!(self.expect_message(MessageId::ResCalibration));

        match msg {
            Message::ResCalibration(_, res) => Ok(res),
            _ => Err(io::Error::new(io::ErrorKind::Other, "unexpected information response"))
        }
    }

    /// Retrieve power buffer
    fn get_power_buffer(&mut self, mac: u64, addr: u32) -> io::Result<ResPowerBuffer> {
        try!(self.send_message( Message::ReqPowerBuffer(ReqHeader{mac: mac},
                                                        ReqPowerBuffer{logaddr: addr})));

        let msg = try!(self.expect_message(MessageId::ResPowerBuffer));

        match msg {
            Message::ResPowerBuffer(_, res) => Ok(res),
            _ => Err(io::Error::new(io::ErrorKind::Other, "unexpected information response"))
        }
    }

    /// Retrieve actual power usage
    fn get_power_usage(&mut self, mac: u64) -> io::Result<ResPowerUse> {
        try!(self.send_message(Message::ReqPowerUse(ReqHeader{mac: mac})));

        let msg = try!(self.expect_message(MessageId::ResPowerUse));

        match msg {
            Message::ResPowerUse(_, res) => Ok(res),
            _ => Err(io::Error::new(io::ErrorKind::Other, "unexpected information response"))
        }
    }

    /// Retrieve actual power usage
    fn get_clock_info(&mut self, mac: u64) -> io::Result<ResClockInfo> {
        try!(self.send_message(Message::ReqClockInfo(ReqHeader{mac: mac})));

        let msg = try!(self.expect_message(MessageId::ResClockInfo));

        match msg {
            Message::ResClockInfo(_, res) => Ok(res),
            _ => Err(io::Error::new(io::ErrorKind::Other, "unexpected information response"))
        }
    }

    /// Set clock
    fn set_clock(&mut self, mac: u64, clock_set: ReqClockSet) -> io::Result<()> {
        try!(self.send_message(Message::ReqClockSet(ReqHeader{mac: mac}, clock_set)));

        try!(self.wait_for_mac_ack(mac));

        Ok(())
    }
}

fn run() -> io::Result<()> {
    let mut path = home_dir().unwrap(); // XXX
    path.push("plugwise.toml");
    let mut file = try!(File::open(&path));
    let mut config = String::new();
    try!(file.read_to_string(&mut config));
    let config = toml::Parser::new(&config).parse().unwrap(); // XXX

    //let mut port = try!(serial::open("/dev/ttyUSB0"));
    //try!(port.configure(|settings| {
    //    settings.set_baud_rate(serial::Baud115200);
    //    settings.set_char_size(serial::Bits8);
    //    settings.set_parity(serial::ParityNone);
    //    settings.set_stop_bits(serial::Stop1);
    //}));

    //port.set_timeout(Duration::milliseconds(1000));
    let port = stub::Stub::new();

    let mut debug = io::stdout();

    let mut plugwise = Protocol::new(port);

    plugwise.set_snoop(ProtocolSnoop::Debug(&mut debug));

    let _ = try!(plugwise.initialize());

    for (_, item) in config {
        if let Some(mac) = item.as_table().unwrap().get("mac") { // XXX
            let mac = mac.as_str().unwrap(); // XXX
            let mac = u64::from_str_radix(mac, 16).unwrap(); // XXX

            try!(plugwise.set_clock(mac, ReqClockSet::new_from_tm(time::now())));

            let info = try!(plugwise.get_info(mac));

            println!("{}", info.datetime.to_tm().unwrap().asctime());

            try!(plugwise.switch(mac, !info.relay_state));
            let _ = try!(plugwise.calibrate(mac));
            let _ = try!(plugwise.get_power_buffer(mac, 0));
            let _ = try!(plugwise.get_power_usage(mac));
            let _ = try!(plugwise.get_clock_info(mac));
        }
    }

    Ok(())
}

fn main() {
    run().unwrap();
}
