extern crate serial;
extern crate time;
extern crate crc16;
extern crate toml;

use std::io;
use std::io::prelude::*;
use serial::prelude::*;
use time::{Timespec, Duration, Tm, now};
use crc16::*;
use std::fs::File;
use std::env::home_dir;
use std::mem::transmute;

const HEADER: [u8; 4] = [5, 5, 3, 3];
const FOOTER: [u8; 2] = [13, 10];
const EOM: u8 = 10;
const CRC_SIZE: usize = 4;
const ADDR_OFFS: u32 = 278528;
const BYTES_PER_POS: u32 = 32;
const PULSES_PER_KWS:f64 = 468.9385193;

/// Convert log element to memory address
fn pos2addr(pos: u32) -> u32 {
    (pos * BYTES_PER_POS) + ADDR_OFFS
}

/// Convert memory address to log element
fn addr2pos(addr: u32) -> u32 {
    (addr - ADDR_OFFS) / BYTES_PER_POS
}

/// Convert pulses to kWs
fn to_kws(pulses: u32) -> f64 {
    pulses as f64 / PULSES_PER_KWS
}

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

    // FIXME: Rust generics doesn't allow yet to create a generic version for u8, u16, u32, u64
 
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

    /// Consume a `f32` from the buffer
    fn decode_f32(&self) -> io::Result<(RawDataConsumer, f32)> {
        let (result, unconverted) = try!(self.decode_u32());
        let converted: f32 = unsafe { transmute(unconverted) };
        Ok((result, converted))
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
        let (result, _) = try!(self.decode_u32());

        let (result1, year) = try!(self.decode_u8());
        let (result1, months) = try!(result1.decode_u8());
        let (_, minutes) = try!(result1.decode_u16());

        Ok((result, DateTime {
            year: year,
            months: months,
            minutes: minutes,
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

impl DateTime {
    fn new(timestamp: Tm) -> DateTime{
        let utc = timestamp.to_utc();

        DateTime {
            year: (utc.tm_year - 100) as u8,
            months: (utc.tm_mon + 1) as u8,
            minutes: (((utc.tm_mday - 1) * 24 * 60) + (utc.tm_hour * 60) + utc.tm_min) as u16
        }
    }

    fn to_tm(&self) -> Option<Tm> {
        let min = (self.minutes % 60) as i32;
        let hours = ((self.minutes / 60) % 24) as i32;
        let mday = 1 + (self.minutes / (24 * 60)) as i32;

        if self.months > 12 || mday > 31 {
            return None;
        }

        let tm = Tm {
            tm_sec: 0,
            tm_min: min,
            tm_hour: hours,
            tm_mday: mday,
            tm_mon: (self.months - 1) as i32,
            tm_year: 100 + (self.year) as i32,
            tm_wday: 0,
            tm_yday: 0,
            tm_isdst: 0,
            tm_utcoff: 0,
            tm_nsec: 0
        };

        Some(tm.to_utc())
    }
}

#[derive(Debug, Clone)]
struct ResInfo {
    datetime: DateTime,
    last_logaddr: u32,
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
            last_logaddr: addr2pos(last_logaddr),
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

#[derive(Debug, Copy, Clone)]
struct ResCalibration {
    gain_a: f32,
    gain_b: f32,
    off_total: f32,
    off_noise: f32
}

impl ResCalibration {
    fn new(decoder: RawDataConsumer) -> io::Result<ResCalibration> {
        let (decoder, gain_a) = try!(decoder.decode_f32());
        let (decoder, gain_b) = try!(decoder.decode_f32());
        let (decoder, off_total) = try!(decoder.decode_f32());
        let (_, off_noise) = try!(decoder.decode_f32());

        Ok(ResCalibration {
            gain_a: gain_a,
            gain_b: gain_b,
            off_total: off_total,
            off_noise: off_noise
        })
    }
}

#[derive(Debug, Copy, Clone)]
struct ReqPowerBuffer {
    logaddr: u32
}

impl ReqPowerBuffer {
    fn as_bytes(&self) -> Vec<u8> {
        let logaddr = pos2addr(self.logaddr);

        format!("{:08X}", logaddr).bytes().collect()
    }
}

#[derive(Debug, Copy, Clone)]
struct ResPowerBuffer {
    datetime1: DateTime,
    pulses1: f64,
    datetime2: DateTime,
    pulses2: f64,
    datetime3: DateTime,
    pulses3: f64,
    datetime4: DateTime,
    pulses4: f64,
    logaddr: u32,
}

impl ResPowerBuffer {
    fn new(decoder: RawDataConsumer) -> io::Result<ResPowerBuffer> {
        let (decoder, datetime1) = try!(decoder.decode_datetime());
        let (decoder, pulses1) = try!(decoder.decode_u32());
        let (decoder, datetime2) = try!(decoder.decode_datetime());
        let (decoder, pulses2) = try!(decoder.decode_u32());
        let (decoder, datetime3) = try!(decoder.decode_datetime());
        let (decoder, pulses3) = try!(decoder.decode_u32());
        let (decoder, datetime4) = try!(decoder.decode_datetime());
        let (decoder, pulses4) = try!(decoder.decode_u32());
        let (_, logaddr) = try!(decoder.decode_u32());

        Ok(ResPowerBuffer {
            datetime1: datetime1,
            pulses1: to_kws(pulses1),
            datetime2: datetime2,
            pulses2: to_kws(pulses2),
            datetime3: datetime3,
            pulses3: to_kws(pulses3),
            datetime4: datetime4,
            pulses4: to_kws(pulses4),
            logaddr: addr2pos(logaddr)
        })
    }
}

#[derive(Debug, Copy, Clone)]
struct ResPowerUse {
    pulse_1s: f64,
    pulse_8s: f64,
    pulse_hour: f64,
    unknown1: u16,
    unknown2: u16,
    unknown3: u16,
}

impl ResPowerUse {
    fn new(decoder: RawDataConsumer) -> io::Result<ResPowerUse> {
        let (decoder, pulse_1s) = try!(decoder.decode_u16());
        let (decoder, pulse_8s) = try!(decoder.decode_u16());
        let (decoder, pulse_hour) = try!(decoder.decode_u32());
        let (decoder, unknown1) = try!(decoder.decode_u16());
        let (decoder, unknown2) = try!(decoder.decode_u16());
        let (_, unknown3) = try!(decoder.decode_u16());

        Ok(ResPowerUse {
            pulse_1s: to_kws(pulse_1s as u32),
            pulse_8s: to_kws(pulse_8s as u32),
            pulse_hour: to_kws(pulse_hour),
            unknown1: unknown1,
            unknown2: unknown2,
            unknown3: unknown3,
        })
    }
}

#[derive(Debug, Copy, Clone)]
struct ResClockInfo {
    hour: u8,
    minute: u8,
    second: u8,
    day_of_week: u8,
    unknown: u16
}

impl ResClockInfo {
    fn new(decoder: RawDataConsumer) -> io::Result<ResClockInfo> {
        let (decoder, hour) = try!(decoder.decode_u8());
        let (decoder, minute) = try!(decoder.decode_u8());
        let (decoder, second) = try!(decoder.decode_u8());
        let (decoder, day_of_week) = try!(decoder.decode_u8());
        let (_, unknown) = try!(decoder.decode_u16());

        Ok(ResClockInfo {
            hour: hour,
            minute: minute,
            second: second,
            day_of_week: day_of_week,
            unknown: unknown
        })
    }
}

#[derive(Debug, Copy, Clone)]
struct ReqClockSet {
    datetime: DateTime,
    logaddr: Option<u32>,
    hour: u8,
    minute: u8,
    second: u8,
    day_of_week: u8,
}

impl ReqClockSet {
    fn new_from_tm(tm: Tm) -> ReqClockSet {
        let utc = tm.to_utc();

        ReqClockSet {
            datetime: DateTime::new(utc),
            logaddr: None,
            hour: utc.tm_hour as u8,
            minute: utc.tm_min as u8,
            second: utc.tm_sec as u8,
            day_of_week: utc.tm_wday as u8,
        }
    }

    fn as_bytes(&self) -> Vec<u8> {
        let logaddr = match self.logaddr {
            None => 0xffffffff,
            Some(addr) => pos2addr(addr)
        };

        format!("{:02X}{:02X}{:04X}{:08X}{:02X}{:02X}{:02X}{:02X}",
                self.datetime.year, self.datetime.months, self.datetime.minutes, logaddr,
                self.hour, self.minute, self.second, self.day_of_week).bytes().collect()
    }
}

const ACK: u16 = 0x0000;
const REQ_INITIALIZE: u16 = 0x000A;
const RES_INITIALIZE: u16 = 0x0011;
const REQ_INFO: u16 = 0x0023;
const RES_INFO: u16 = 0x0024;
const REQ_SWITCH: u16 = 0x0017;
const REQ_CALIBRATION: u16 = 0x0026;
const RES_CALIBRATION: u16 = 0x0027;
const REQ_POWER_BUFFER: u16 = 0x0048;
const RES_POWER_BUFFER: u16 = 0x0049;
const REQ_POWER_USE: u16 = 0x0012;
const RES_POWER_USE: u16 = 0x0013;
const REQ_CLOCK_INFO: u16 = 0x003E;
const RES_CLOCK_INFO: u16 = 0x003F;
const REQ_CLOCK_SET: u16 = 0x0016;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u16)]
enum MessageId {
    Ack = ACK,
    ReqInitialize = REQ_INITIALIZE,
    ResInitialize = RES_INITIALIZE,
    ReqInfo = REQ_INFO,
    ResInfo = RES_INFO,
    ReqSwitch = REQ_SWITCH,
    ReqCalibration = REQ_CALIBRATION,
    ResCalibration = RES_CALIBRATION,
    ReqPowerBuffer = REQ_POWER_BUFFER,
    ResPowerBuffer = RES_POWER_BUFFER,
    ReqPowerUse = REQ_POWER_USE,
    ResPowerUse = RES_POWER_USE,
    ReqClockInfo = REQ_CLOCK_INFO,
    ResClockInfo = RES_CLOCK_INFO,
    ReqClockSet = REQ_CLOCK_SET,
}

impl MessageId {
    fn new(id: u16) -> MessageId {
        match id {
            ACK => MessageId::Ack,
            REQ_INITIALIZE => MessageId::ReqInitialize,
            RES_INITIALIZE => MessageId::ResInitialize,
            REQ_INFO => MessageId::ReqInfo,
            RES_INFO => MessageId::ResInfo,
            REQ_SWITCH => MessageId::ReqSwitch,
            REQ_CALIBRATION => MessageId::ReqCalibration,
            RES_CALIBRATION => MessageId::ResCalibration,
            REQ_POWER_BUFFER => MessageId::ReqPowerBuffer,
            RES_POWER_BUFFER => MessageId::ResPowerBuffer,
            REQ_POWER_USE => MessageId::ReqPowerUse,
            RES_POWER_USE => MessageId::ResPowerUse,
            REQ_CLOCK_INFO => MessageId::ReqClockInfo,
            RES_CLOCK_INFO => MessageId::ResClockInfo,
            REQ_CLOCK_SET => MessageId::ReqClockSet,
            _ => MessageId::Ack
        }
    }

    fn as_bytes(&self) -> Vec<u8> {
        format!("{:04X}", *self as u16).bytes().collect()
    }
}

#[derive(Debug, Clone)]
enum Message {
    Ack(ResHeader),
    ReqInitialize,
    ResInitialize(ResHeader, ResInitialize),
    ReqInfo(ReqHeader),
    ResInfo(ResHeader, ResInfo),
    ReqSwitch(ReqHeader, ReqSwitch),
    ReqCalibration(ReqHeader),
    ResCalibration(ResHeader, ResCalibration),
    ReqPowerBuffer(ReqHeader, ReqPowerBuffer),
    ResPowerBuffer(ResHeader, ResPowerBuffer),
    ReqPowerUse(ReqHeader),
    ResPowerUse(ResHeader, ResPowerUse),
    ReqClockInfo(ReqHeader),
    ResClockInfo(ResHeader, ResClockInfo),
    ReqClockSet(ReqHeader, ReqClockSet),
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
            Message::ReqSwitch(header, _) |
            Message::ReqCalibration(header) |
            Message::ReqPowerBuffer(header, _) |
            Message::ReqPowerUse(header) |
            Message::ReqClockInfo(header) |
            Message::ReqClockSet(header, _) => vec.extend(header.as_bytes()),
            _ => {}
        }

        match *self {
            Message::ReqInitialize |
            Message::ReqInfo(_) |
            Message::ReqCalibration(_) |
            Message::ReqPowerUse(_) |
            Message::ReqClockInfo(_) => Ok(vec),
            Message::ReqPowerBuffer(_, req) => {
                vec.extend(req.as_bytes());
                Ok(vec)
            },
            Message::ReqSwitch(_, req) => {
                vec.extend(req.as_bytes());
                Ok(vec)
            },
            Message::ReqClockSet(_, req) => {
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

        let (decoder, mac) = if msg_id != MessageId::Ack {
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
            MessageId::ResCalibration =>
                Ok(Message::ResCalibration(header, try!(ResCalibration::new(decoder)))),
            MessageId::ResPowerBuffer =>
                Ok(Message::ResPowerBuffer(header, try!(ResPowerBuffer::new(decoder)))),
            MessageId::ResPowerUse =>
                Ok(Message::ResPowerUse(header, try!(ResPowerUse::new(decoder)))),
            MessageId::ResClockInfo =>
                Ok(Message::ResClockInfo(header, try!(ResClockInfo::new(decoder)))),
            _ => 
                Ok(Message::Ack(header))
        }
    }

    fn to_message_id(&self) -> MessageId {
        match *self {
            Message::Ack(..) => MessageId::Ack,
            Message::ReqInitialize(..) => MessageId::ReqInitialize,
            Message::ResInitialize(..) => MessageId::ResInitialize,
            Message::ReqInfo(..) => MessageId::ReqInfo,
            Message::ResInfo(..) => MessageId::ResInfo,
            Message::ReqSwitch(..) => MessageId::ReqSwitch,
            Message::ReqCalibration(..) => MessageId::ReqCalibration,
            Message::ResCalibration(..) => MessageId::ResCalibration,
            Message::ReqPowerBuffer(..) => MessageId::ReqPowerBuffer,
            Message::ResPowerBuffer(..) => MessageId::ResPowerBuffer,
            Message::ReqPowerUse(..) => MessageId::ReqPowerUse,
            Message::ResPowerUse(..) => MessageId::ResPowerUse,
            Message::ReqClockInfo(..) => MessageId::ReqClockInfo,
            Message::ResClockInfo(..) => MessageId::ResClockInfo,
            Message::ReqClockSet(..) => MessageId::ReqClockSet,
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
