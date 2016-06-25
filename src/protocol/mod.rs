mod messages;

use std::io;
use std::io::prelude::*;
use crc16::*;
pub use self::messages::{ReqClockSet, ResInitialize, ResInfo,
                         ResCalibration, ResPowerBuffer, ResPowerUse,
                         ResClockInfo, DateTime, Pulses};
use self::messages::{Message, MessageId, ReqHeader, ReqSwitch, ReqPowerBuffer};
use super::error;

const HEADER: [u8; 4] = [5, 5, 3, 3];
const FOOTER: [u8; 2] = [13, 10];
const EOM: u8 = 10;
const CRC_SIZE: usize = 4;
const DEFAULT_RETRIES: u8 = 3;

/// Plugwise communication snooper setting.
pub enum ProtocolSnoop<'a> {
    /// Log nothing (default).
    Nothing,
    /// Log developer readable data of the Plugwise communication.
    Debug(&'a mut Write),
    /// Log the relevant raw serial communication of the Plugwise communication.
    Raw(&'a mut Write),
    /// Log all raw serial communication of the Plugwise communication (very verbose, which
    /// actually doesn't make much sense, unless you're a developer of Plugwise devices).
    All(&'a mut Write)
}

pub struct Protocol<'a, R> {
    reader: io::BufReader<R>,
    snoop: ProtocolSnoop<'a>,
    retries: u8,
}

impl<'a, R: Read + Write> Protocol<'a, R> {
    /// Wrap IO entity for Plugwise protocol handling
    pub fn new(port: R) -> Protocol<'a, R> {
        Protocol {
            reader: io::BufReader::with_capacity(1000, port),
            snoop: ProtocolSnoop::Nothing,
            retries: DEFAULT_RETRIES,
        }
    }

    pub fn set_retries(&mut self, retries: u8) {
        self.retries = retries;
    }

    pub fn set_snoop(&mut self, snoop: ProtocolSnoop<'a>) {
        self.snoop = snoop;
    }

    /// Send payload
    fn send_message_raw(&mut self, payload: &[u8]) -> error::PlResult<()> {
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
    fn receive_plugwise_message_raw(&mut self) -> error::PlResult<(Vec<u8>, usize, usize)> {
        loop {
            let mut buf = vec![];

            let _ = try!(self.reader.read_until(EOM, &mut buf));

            let header_pos = buf.windows(HEADER.len()).position(|x| *x==HEADER);

            if header_pos.is_some() {
                let header_pos = header_pos.unwrap(); // that would be a surprise when this panics

                let footer_pos = match buf.windows(FOOTER.len()).rposition(|x| *x==FOOTER){
                    None => return Err(error::PlError::Protocol),
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
    fn receive_message_raw(&mut self) -> error::PlResult<Vec<u8>> {
        let (buf, header_pos, footer_pos) = try!(self.receive_plugwise_message_raw());

        // chop off header, footer and CRC
        let payload = buf.iter().take(footer_pos - CRC_SIZE).skip(header_pos + HEADER.len());
        let crc = buf.iter().skip(footer_pos - CRC_SIZE).take(CRC_SIZE);
        let crc = crc.take(4).fold(0, |acc, &item| {
            acc << 4 | (item as char).to_digit(16).unwrap_or_default() as u16
        });

        // CRC check
        let mut state = State::<XMODEM>::new();
        for byte in payload {
            state.update(&[*byte]);
        }

        if crc != state.get() {
            return Err(error::PlError::Protocol);
        }

        let payload = buf.iter().take(footer_pos - CRC_SIZE).skip(header_pos + HEADER.len());

        Ok(payload.cloned().collect())
    }

    /// Keep receiving messages until the given message identifier has been received
    fn expect_message(&mut self, expected_message_id: MessageId) -> error::PlResult<Message> {
        loop {
            let msg = try!(self.receive_message_raw());
            let msg = try!(Message::from_payload(&msg));

            debug!("received: {:?}", msg);

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

    fn wait_for_mac_ack(&mut self, expected_mac: u64) -> error::PlResult<()> {
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
    fn send_message(&mut self, message: &Message) -> error::PlResult<()> {
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

    /// Send a message and wait for response
    fn send_and_expect(&mut self, message: Message, expected: MessageId) -> error::PlResult<Message> {
        let mut retries = self.retries;

        loop {
            try!(self.send_message(&message));
            match self.expect_message(expected) {
                Ok(n) => return Ok(n),
                Err(e) => {
                    if retries == 0 {
                        return Err(e);
                    } else if let error::PlError::Io(e) = e {
                        if e.kind() != io::ErrorKind::TimedOut {
                            return Err(error::PlError::Io(e));
                        } else {
                            retries = retries - 1;
                        }
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    /// Send a message and wait for acknowledge with a mac
    fn send_and_expect_ack(&mut self, message: Message, mac: u64) -> error::PlResult<()> {
        let mut retries = self.retries;

        loop {
            try!(self.send_message(&message));
            debug!("sending {:?}", message);
            match self.wait_for_mac_ack(mac) {
                Ok(n) => {
                    return Ok(n)
                }
                Err(e) => {
                    if retries == 0 {
                        return Err(e);
                    } else if let error::PlError::Io(e) = e {
                        if e.kind() != io::ErrorKind::TimedOut {
                            return Err(error::PlError::Io(e));
                        } else {
                            retries = retries - 1;
                        }
                    } else {
                        return Err(e);
                    }
                    info!("retries pending {} for {:?}", retries, message);
                }
            }
        }
    }

    /// Initialize the Plugwise USB stick
    pub fn initialize(&mut self) -> error::PlResult<ResInitialize> {
        let msg = try!(self.send_and_expect(Message::ReqInitialize,
                                            MessageId::ResInitialize));

        match msg {
            Message::ResInitialize(_, res) => Ok(res),
            _ => Err(error::PlError::UnexpectedResponse)
        }
    }

    /// Get info from a circle
    pub fn get_info(&mut self, mac: u64) -> error::PlResult<ResInfo> {
        let msg = try!(self.send_and_expect(Message::ReqInfo(ReqHeader{mac: mac}),
                                            MessageId::ResInfo));

        match msg {
            Message::ResInfo(_, res) => Ok(res),
            _ => Err(error::PlError::UnexpectedResponse)
        }
    }

    /// Switch a circle
    pub fn switch(&mut self, mac: u64, on: bool) -> error::PlResult<()> {
        try!(self.send_and_expect_ack(Message::ReqSwitch(ReqHeader{mac: mac},
                                                         ReqSwitch{on: on}),
                                      mac));
        Ok(())
    }

    /// Calibrate a circle
    pub fn calibrate(&mut self, mac: u64) -> error::PlResult<ResCalibration> {
        let msg = try!(self.send_and_expect(Message::ReqCalibration(ReqHeader{mac: mac}),
                                            MessageId::ResCalibration));

        match msg {
            Message::ResCalibration(_, res) => Ok(res),
            _ => Err(error::PlError::UnexpectedResponse)
        }
    }

    /// Retrieve power buffer
    pub fn get_power_buffer(&mut self, mac: u64, addr: u32) -> error::PlResult<ResPowerBuffer> {
        let msg = try!(self.send_and_expect(Message::ReqPowerBuffer(ReqHeader{mac: mac},
                                                                    ReqPowerBuffer{logaddr: addr}),
                                            MessageId::ResPowerBuffer));

        match msg {
            Message::ResPowerBuffer(_, res) => Ok(res),
            _ => Err(error::PlError::UnexpectedResponse)
        }
    }

    /// Retrieve actual power usage
    pub fn get_power_usage(&mut self, mac: u64) -> error::PlResult<ResPowerUse> {
        let msg = try!(self.send_and_expect(Message::ReqPowerUse(ReqHeader{mac: mac}),
                                            MessageId::ResPowerUse));

        match msg {
            Message::ResPowerUse(_, res) => Ok(res),
            _ => Err(error::PlError::UnexpectedResponse)
        }
    }

    /// Retrieve actual power usage
    pub fn get_clock_info(&mut self, mac: u64) -> error::PlResult<ResClockInfo> {
        let msg = try!(self.send_and_expect(Message::ReqClockInfo(ReqHeader{mac: mac}),
                                            MessageId::ResClockInfo));

        match msg {
            Message::ResClockInfo(_, res) => Ok(res),
            _ => Err(error::PlError::UnexpectedResponse)
        }
    }

    /// Set clock
    pub fn set_clock(&mut self, mac: u64, clock_set: ReqClockSet) -> error::PlResult<()> {
        try!(self.send_and_expect_ack(Message::ReqClockSet(ReqHeader{mac: mac},
                                                           clock_set),
                                      mac));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // Note that these test relies on that the implemention and stub yields
    // errors and panics when something strange happens.

    use super::super::stub;
    use super::*;
    use time;

    #[test]
    fn stub_initialize() {
        let port = stub::Stub::new();
        let mut protocol = Protocol::new(port);

        assert_eq!(true, protocol.initialize().unwrap().is_online);
    }

    #[test]
    fn stub_switch_and_info() {
        let mac1 = 0x0123456789abcdef;
        let mac2 = 0xfedcba9876543210;
        let port = stub::Stub::new();
        let mut protocol = Protocol::new(port);

        let info1 = protocol.get_info(mac1).unwrap();
        let info2 = protocol.get_info(mac2).unwrap();

        assert_eq!(false, info1.relay_state);
        assert_eq!(false, info2.relay_state);

        protocol.switch(mac1, true).unwrap();
        protocol.switch(mac2, false).unwrap();

        let info1 = protocol.get_info(mac1).unwrap();
        let info2 = protocol.get_info(mac2).unwrap();

        assert_eq!(true, info1.relay_state);
        assert_eq!(false, info2.relay_state);

        protocol.switch(mac2, true).unwrap();

        let info1 = protocol.get_info(mac1).unwrap();
        let info2 = protocol.get_info(mac2).unwrap();

        assert_eq!(true, info1.relay_state);
        assert_eq!(true, info2.relay_state);

        protocol.switch(mac1, false).unwrap();

        let info1 = protocol.get_info(mac1).unwrap();
        let info2 = protocol.get_info(mac2).unwrap();

        assert_eq!(false, info1.relay_state);
        assert_eq!(true, info2.relay_state);
    }

    #[test]
    fn stub_set_clock() {
        let mac = 0x0123456789abcdef;
        let port = stub::Stub::new();
        let mut protocol = Protocol::new(port);

        protocol.set_clock(mac, ReqClockSet::new_from_tm(time::now())).unwrap();
    }

    #[test]
    fn stub_calibrate() {
        let mac = 0x0123456789abcdef;
        let port = stub::Stub::new();
        let mut protocol = Protocol::new(port);

        let _ = protocol.calibrate(mac).unwrap();
    }

    #[test]
    fn stub_get_power_buffer() {
        let mac = 0x0123456789abcdef;
        let port = stub::Stub::new();
        let mut protocol = Protocol::new(port);

        let _ = protocol.get_power_buffer(mac, 0).unwrap();
    }

    #[test]
    fn stub_get_power_usage() {
        let mac = 0x0123456789abcdef;
        let port = stub::Stub::new();
        let mut protocol = Protocol::new(port);

        let _ = protocol.get_power_usage(mac).unwrap();
    }

    #[test]
    fn stub_get_clock_info() {
        let mac = 0x0123456789abcdef;
        let port = stub::Stub::new();
        let mut protocol = Protocol::new(port);

        let _ = protocol.get_clock_info(mac).unwrap();
    }
}
