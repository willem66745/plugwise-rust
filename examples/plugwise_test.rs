extern crate serial;
extern crate time;
extern crate crc16;
extern crate toml;

mod messages;

use std::io;
use std::io::prelude::*;
use serial::prelude::*;
use time::Duration;
use crc16::*;
use std::fs::File;
use std::env::home_dir;
use messages::*;

const HEADER: [u8; 4] = [5, 5, 3, 3];
const FOOTER: [u8; 2] = [13, 10];
const EOM: u8 = 10;
const CRC_SIZE: usize = 4;

struct Protocol<R> {
    reader: io::BufReader<R>
}

impl<R: Read + Write> Protocol<R> {
    /// Wrap IO entity for Plugwise protocol handling
    fn new(port: R) -> Protocol<R> {
        Protocol {
            reader: io::BufReader::with_capacity(1000, port)
        }
    }

    /// Send payload
    fn send_message_raw(&mut self, payload: &[u8]) -> io::Result<()> {
        let crc = format!("{:04X}", State::<XMODEM>::calculate(payload)).into_bytes();

        try!(self.reader.get_mut().write(&HEADER));
        try!(self.reader.get_mut().write(payload));
        try!(self.reader.get_mut().write(&crc));
        try!(self.reader.get_mut().write(&FOOTER));

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

                return Ok((buf, header_pos, footer_pos))
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

            println!("< {:?}", msg); // XXX

            if msg.to_message_id() == expected_message_id {
                return Ok(msg)
            }
        }
    }

    /// Initialize the Plugwise USB stick
    fn initialize(&mut self) -> io::Result<ResInitialize> {
        let msg = try!(Message::ReqInitialize.to_payload());
        try!(self.send_message_raw(&msg));

        let msg = try!(self.expect_message(MessageId::ResInitialize));

        match msg {
            Message::ResInitialize(_, res) => Ok(res),
            _ => Err(io::Error::new(io::ErrorKind::Other, "unexpected initialization response"))
        }
    }

    /// Get info from a circle
    fn get_info(&mut self, mac: u64) -> io::Result<ResInfo> {
        let msg = try!(Message::ReqInfo(ReqHeader{mac: mac}).to_payload());
        try!(self.send_message_raw(&msg));

        let msg = try!(self.expect_message(MessageId::ResInfo));

        match msg {
            Message::ResInfo(_, res) => Ok(res),
            _ => Err(io::Error::new(io::ErrorKind::Other, "unexpected information response"))
        }
    }

    /// Switch a circle
    fn switch(&mut self, mac: u64, on: bool) -> io::Result<()> {
        let msg = try!(Message::ReqSwitch(ReqHeader{mac: mac}, ReqSwitch{on: on}).to_payload());
        try!(self.send_message_raw(&msg));

        let _ = try!(self.expect_message(MessageId::Ack));

        Ok(())
    }

    /// Calibrate a circle
    fn calibrate(&mut self, mac: u64) -> io::Result<ResCalibration> {
        let msg = try!(Message::ReqCalibration(ReqHeader{mac: mac}).to_payload());
        try!(self.send_message_raw(&msg));

        let msg = try!(self.expect_message(MessageId::ResCalibration));

        match msg {
            Message::ResCalibration(_, res) => Ok(res),
            _ => Err(io::Error::new(io::ErrorKind::Other, "unexpected information response"))
        }
    }

    /// Retrieve power buffer
    fn get_power_buffer(&mut self, mac: u64, addr: u32) -> io::Result<ResPowerBuffer> {
        let msg = try!(Message::ReqPowerBuffer(ReqHeader{mac: mac},
                                               ReqPowerBuffer{logaddr: addr}).to_payload());

        try!(self.send_message_raw(&msg));

        let msg = try!(self.expect_message(MessageId::ResPowerBuffer));

        match msg {
            Message::ResPowerBuffer(_, res) => Ok(res),
            _ => Err(io::Error::new(io::ErrorKind::Other, "unexpected information response"))
        }
    }

    /// Retrieve actual power usage
    fn get_power_usage(&mut self, mac: u64) -> io::Result<ResPowerUse> {
        let msg = try!(Message::ReqPowerUse(ReqHeader{mac: mac}).to_payload());
        try!(self.send_message_raw(&msg));

        let msg = try!(self.expect_message(MessageId::ResPowerUse));

        match msg {
            Message::ResPowerUse(_, res) => Ok(res),
            _ => Err(io::Error::new(io::ErrorKind::Other, "unexpected information response"))
        }
    }

    /// Retrieve actual power usage
    fn get_clock_info(&mut self, mac: u64) -> io::Result<ResClockInfo> {
        let msg = try!(Message::ReqClockInfo(ReqHeader{mac: mac}).to_payload());
        try!(self.send_message_raw(&msg));

        let msg = try!(self.expect_message(MessageId::ResClockInfo));

        match msg {
            Message::ResClockInfo(_, res) => Ok(res),
            _ => Err(io::Error::new(io::ErrorKind::Other, "unexpected information response"))
        }
    }

    /// Set clock
    fn set_clock(&mut self, mac: u64, clock_set: ReqClockSet) -> io::Result<()> {
        let msg = try!(Message::ReqClockSet(ReqHeader{mac: mac}, clock_set).to_payload());

        try!(self.send_message_raw(&msg));

        let msg = try!(self.expect_message(MessageId::Ack));

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

    let mut port = try!(serial::open("/dev/ttyUSB0"));
    try!(port.configure(|settings| {
        settings.set_baud_rate(serial::Baud115200);
        settings.set_char_size(serial::Bits8);
        settings.set_parity(serial::ParityNone);
        settings.set_stop_bits(serial::Stop1);
    }));

    port.set_timeout(Duration::milliseconds(1000));

    let mut plugwise = Protocol::new(port);

    let _ = try!(plugwise.initialize());

    for (_, item) in config {
        if let Some(mac) = item.as_table().unwrap().get("mac") { // XXX
            let mac = mac.as_str().unwrap(); // XXX
            let mac = u64::from_str_radix(mac, 16).unwrap(); // XXX

            //try!(plugwise.set_clock(mac, ReqClockSet::new_from_tm(now())));

            let info = try!(plugwise.get_info(mac));

            println!("{}", info.datetime.to_tm().unwrap().asctime());

            //try!(plugwise.switch(mac, !info.relay_state));
            //let _ = try!(plugwise.calibrate(mac));
            //let _ = try!(plugwise.get_power_buffer(mac, 0));
            //let _ = try!(plugwise.get_power_usage(mac));
            let _ = try!(plugwise.get_clock_info(mac));
        }
    }

    Ok(())
}

fn main() {
    run().unwrap();
}
