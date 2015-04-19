extern crate serial;
extern crate time;
extern crate crc16;
extern crate toml;

use std::io;
use std::io::prelude::*;
use serial::prelude::*;
use time::{Timespec, Duration};
use crc16::*;
use std::fs::File;
use std::env::home_dir;

const HEADER: [u8; 4] = [5, 5, 3, 3];
const FOOTER: [u8; 2] = [13, 10];
const EOM: u8 = 10;
const CRC_SIZE: usize = 4;

/// Plugwise raw data consumer
struct RawDataConsumer<'a> {
    buf: &'a[u8],
}

impl<'a> RawDataConsumer<'a> {
    /// Wrap a buffer to consumer
    fn new(buf: &'a[u8]) -> RawDataConsumer<'a> {
        RawDataConsumer {
            buf: buf,
        }
    }

    /// Consume the buffer and create a new instance of the consumer
    fn consume(&self, size: usize) -> io::Result<(&'a[u8], RawDataConsumer)> {
        if (self.buf.len()) < size {
            return Err(io::Error::new(io::ErrorKind::Other, "data missing in received message"));
        }

        let (value, remainder) = self.buf.split_at(size);

        Ok((value, RawDataConsumer {
            buf: remainder,
        }))
    }

    // TODO: Rust generics doesn't allow yet to create a generic version for u8, u16, u32, u64
 
    /// Consume a `u8` from the buffer
    fn decode_u8(&self) -> io::Result<(RawDataConsumer, u8)> {
        let (buf, result) = try!(self.consume(2));
        let mut value = 0;

        for byte in buf {
            value = value << 4 | match *byte {
                b'0' => 0, b'1' => 1, b'2' => 2,  b'3' => 3,  b'4' => 4,  b'5' => 5,  b'6' => 6,  b'7' => 7,
                b'8' => 8, b'9' => 9, b'A' => 10, b'B' => 11, b'C' => 12, b'D' => 13, b'E' => 14, b'F' => 15,
                _ => 0
            };
        }

        Ok((result, value))
    }

    /// Consume a `u16` from the buffer
    fn decode_u16(&self) -> io::Result<(RawDataConsumer, u16)> {
        let (buf, result) = try!(self.consume(4));
        let mut value = 0;

        for byte in buf {
            value = value << 4 | match *byte {
                b'0' => 0, b'1' => 1, b'2' => 2,  b'3' => 3,  b'4' => 4,  b'5' => 5,  b'6' => 6,  b'7' => 7,
                b'8' => 8, b'9' => 9, b'A' => 10, b'B' => 11, b'C' => 12, b'D' => 13, b'E' => 14, b'F' => 15,
                _ => 0
            };
        }

        Ok((result, value))
    }

    /// Consume a `u32` from the buffer
    fn decode_u32(&self) -> io::Result<(RawDataConsumer, u32)> {
        let (buf, result) = try!(self.consume(8));
        let mut value = 0;

        for byte in buf {
            value = value << 4 | match *byte {
                b'0' => 0, b'1' => 1, b'2' => 2,  b'3' => 3,  b'4' => 4,  b'5' => 5,  b'6' => 6,  b'7' => 7,
                b'8' => 8, b'9' => 9, b'A' => 10, b'B' => 11, b'C' => 12, b'D' => 13, b'E' => 14, b'F' => 15,
                _ => 0
            };
        }

        Ok((result, value))
    }

    /// Consume a `u64` from the buffer
    fn decode_u64(&self) -> io::Result<(RawDataConsumer, u64)> {
        let (buf, result) = try!(self.consume(16));
        let mut value = 0;

        for byte in buf {
            value = value << 4 | match *byte {
                b'0' => 0, b'1' => 1, b'2' => 2,  b'3' => 3,  b'4' => 4,  b'5' => 5,  b'6' => 6,  b'7' => 7,
                b'8' => 8, b'9' => 9, b'A' => 10, b'B' => 11, b'C' => 12, b'D' => 13, b'E' => 14, b'F' => 15,
                _ => 0
            };
        }

        Ok((result, value))
    }

    /// Consume a string of a given size from the buffer
    fn decode_string(&self, size: usize) -> io::Result<(RawDataConsumer, &'a str)> {
        let (buf, result) = try!(self.consume(size));

        match std::str::from_utf8(buf) {
            Ok(text) => Ok((result, text)),
            Err(err) => Err(io::Error::new(io::ErrorKind::Other, err))
        }
    }

    // /// Get the remainder of the data as a string
    // fn remainder(&self) -> io::Result<&'a str> {
    //     match std::str::from_utf8(self.buf) {
    //         Ok(value) => Ok(value),
    //         Err(err) => Err(io::Error::new(io::ErrorKind::Other, err))
    //     }
    // }

    fn decode_datetime(&self) -> io::Result<(RawDataConsumer, DateTime)> {
        let (result, raw_datetime) = try!(self.decode_u32());

        Ok((result, DateTime {
            year: (raw_datetime & 0xff000000 >> 24) as u8,
            months: (raw_datetime & 0x00ff0000 >> 16) as u8,
            minutes: (raw_datetime & 0x0000ffff) as u16
        }))
    }
}

#[derive(Debug, Copy, Clone)]
struct ResHeader {
    msgid: MessageId,
    count: u16,
    mac: u64
}

#[derive(Debug, Copy, Clone)]
struct ReqHeader {
    mac: u64
}

impl ReqHeader {
    fn as_bytes(&self) -> Vec<u8> {
        format!("{:016X}", self.mac).bytes().collect()
    }
}

#[derive(Debug, Copy, Clone)]
struct ResInitialize {
    unknown1: u8,
    is_online: bool,
    network_id: u64,
    short_id: u16,
    unknown2:  u8
}

impl ResInitialize {
    /// Decode initialization response
    fn new(decoder: RawDataConsumer) -> io::Result<ResInitialize> {
        let (decoder, unknown1) = try!(decoder.decode_u8());
        let (decoder, is_online) = try!(decoder.decode_u8());
        let (decoder, network_id) = try!(decoder.decode_u64());
        let (decoder, short_id) = try!(decoder.decode_u16());
        let (_, unknown2) = try!(decoder.decode_u8());

        Ok(ResInitialize {
            unknown1: unknown1,
            is_online: is_online != 0,
            network_id: network_id,
            short_id: short_id,
            unknown2: unknown2,
        })
    }
}

#[derive(Debug, Copy, Clone)]
struct DateTime {
    year: u8,
    months: u8,
    minutes: u16
}

#[derive(Debug, Clone)]
struct ResInfo {
    datetime: DateTime,
    last_logaddr: u64,
    relay_state: bool,
    hz: u8,
    hw_ver: String,
    fw_ver: Timespec,
    unknown: u8
}

impl ResInfo {
    /// Decode info response
    fn new(decoder: RawDataConsumer) -> io::Result<ResInfo> {
        let (decoder, datetime) = try!(decoder.decode_datetime());
        let (decoder, last_logaddr) = try!(decoder.decode_u32());
        let (decoder, relay_state) = try!(decoder.decode_u8());
        let (decoder, hz) = try!(decoder.decode_u8());
        let (decoder, hw_ver) = try!(decoder.decode_string(12));
        let (decoder, fw_ver) = try!(decoder.decode_u32());
        let (_, unknown) = try!(decoder.decode_u8());

        Ok(ResInfo {
            datetime: datetime,
            last_logaddr: last_logaddr as u64 * 32 + 278528, // XXX
            relay_state: relay_state != 0,
            hz: match hz {
                133 => 50,
                197 => 60,
                _ => 0
            },
            hw_ver: hw_ver.to_string(),
            fw_ver: Timespec::new((fw_ver as i32) as i64, 0),
            unknown: unknown
        })
    }
}

#[derive(Debug, Copy, Clone)]
struct ReqSwitch{
    on: bool
}

impl ReqSwitch {
    fn as_bytes(&self) -> Vec<u8> {
        let on = if self.on {1} else {0};

        format!("{:02X}", on).bytes().collect()
    }
}

const EMPTY: isize = 0x0000;
const REQ_INITIALIZE: isize = 0x000A;
const RES_INITIALIZE: isize = 0x0011;
const REQ_INFO: isize = 0x0023;
const RES_INFO: isize = 0x0024;
const REQ_SWITCH: isize = 0x0017;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum MessageId {
    Empty = EMPTY,
    ReqInitialize = REQ_INITIALIZE,
    ResInitialize = RES_INITIALIZE,
    ReqInfo = REQ_INFO,
    ResInfo = RES_INFO,
    ReqSwitch = REQ_SWITCH,
}

impl MessageId {
    fn new(id: u16) -> MessageId {
        match id as isize {
            EMPTY => MessageId::Empty,
            REQ_INITIALIZE => MessageId::ReqInitialize,
            RES_INITIALIZE => MessageId::ResInitialize,
            REQ_INFO => MessageId::ReqInfo,
            RES_INFO => MessageId::ResInfo,
            REQ_SWITCH => MessageId::ReqSwitch,
            _ => MessageId::Empty
        }
    }

    fn as_bytes(&self) -> Vec<u8> {
        format!("{:04X}", *self as u16).bytes().collect()
    }
}

#[derive(Debug, Clone)]
enum Message {
    Empty(ResHeader),
    ReqInitialize,
    ResInitialize(ResHeader, ResInitialize),
    ReqInfo(ReqHeader),
    ResInfo(ResHeader, ResInfo),
    ReqSwitch(ReqHeader, ReqSwitch),
}

impl Message {
    /// Convert given message to a bunch of bytes
    fn to_payload(&self) -> io::Result<Vec<u8>> {
        let mut vec = vec![];

        println!("> {:?}", *self); // XXX

        vec.extend(self.to_message_id().as_bytes());

        // handle header (generically)
        match *self {
            Message::ReqInfo(header) |
            Message::ReqSwitch(header, _) => vec.extend(header.as_bytes()),
            _ => {}
        }

        match *self {
            Message::ReqInitialize | Message::ReqInfo(_) => Ok(vec),
            Message::ReqSwitch(_, req) => {
                vec.extend(req.as_bytes());
                Ok(vec)
            },
            _ => Err(io::Error::new(io::ErrorKind::Other, "Unsupported message type"))
        }
    }

    /// Convert given bunch of bytes to interpretable message
    fn from_payload(payload: &[u8]) -> io::Result<Message> {
        let decoder = RawDataConsumer::new(payload);

        let (decoder, msg_id) = try!(decoder.decode_u16());
        let (decoder, counter) = try!(decoder.decode_u16());
        let msg_id = MessageId::new(msg_id);

        let (decoder, mac) = if msg_id != MessageId::Empty {
            try!(decoder.decode_u64())
        } else {
            (decoder, 0)
        };

        let header = ResHeader {
            msgid: msg_id,
            count: counter,
            mac: mac
        };

        match msg_id {
            MessageId::ResInitialize => 
                Ok(Message::ResInitialize(header, try!(ResInitialize::new(decoder)))),
            MessageId::ResInfo => 
                Ok(Message::ResInfo(header, try!(ResInfo::new(decoder)))),
            _ => 
                Ok(Message::Empty(header))
        }
    }

    fn to_message_id(&self) -> MessageId {
        match *self {
            Message::Empty(..) => MessageId::Empty,
            Message::ReqInitialize(..) => MessageId::ReqInitialize,
            Message::ResInitialize(..) => MessageId::ResInitialize,
            Message::ReqInfo(..) => MessageId::ReqInfo,
            Message::ResInfo(..) => MessageId::ResInfo,
            Message::ReqSwitch(..) => MessageId::ReqSwitch,
        }
    }
}

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
    fn initialize(&mut self) -> io::Result<bool> {
        let msg = try!(Message::ReqInitialize.to_payload());
        try!(self.send_message_raw(&msg));

        let msg = try!(self.expect_message(MessageId::ResInitialize));

        match msg {
            Message::ResInitialize(_, res) => Ok(res.is_online),
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

        let _ = try!(self.expect_message(MessageId::Empty));

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

            let info = try!(plugwise.get_info(mac));

            try!(plugwise.switch(mac, !info.relay_state));
        }
    }

    Ok(())
}

fn main() {
    run().unwrap();
}
