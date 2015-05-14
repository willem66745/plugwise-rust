mod raw;

use time::{Tm, Timespec};
use super::super::error;

const ADDR_OFFS: u32 = 278528;
const BYTES_PER_POS: u32 = 32;
const PULSES_PER_KW: f64 = 468.9385193;

/// Convert log element to memory address
fn pos2addr(pos: u32) -> u32 {
    (pos * BYTES_PER_POS) + ADDR_OFFS
}

/// Convert memory address to log element
fn addr2pos(addr: u32) -> u32 {
    (addr - ADDR_OFFS) / BYTES_PER_POS
}

#[derive(Debug, Copy, Clone)]
pub struct Pulses {
    pulses: u32,
    timespan: u32
}

impl Pulses {
    pub fn new(pulses: u32, timespan: u32) -> Pulses {
        Pulses {
            pulses: pulses,
            timespan: timespan
        }
    }

    /// Retrieve corrected number of pulses per second
    fn to_pulses_per_second(&self, calibration: ResCalibration) -> f64 {
        if self.pulses == 0 || self.pulses == 0xffff {
            0.0
        } else {
            let noise_corrected = (self.pulses as f64 / self.timespan as f64) +
                calibration.off_noise as f64;
            (noise_corrected.powi(2) * calibration.gain_b as f64) +
                (noise_corrected * calibration.gain_a as f64) + calibration.off_total as f64
        }
    }

    /// Convert pulses to kW
    fn to_kw(&self, calibration: ResCalibration) -> f64 {
        let pulses = self.to_pulses_per_second(calibration);
        pulses / PULSES_PER_KW
    }

    /// Convert to Watts
    pub fn to_watts(&self, calibration: ResCalibration) -> f64 {
        self.to_kw(calibration) * 1000.0
    }

    /// Convert to kWh
    pub fn to_kwh(&self, calibration: ResCalibration) -> f64 {
        self.to_kw(calibration) * (self.timespan as f64 / 3600.0)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ResHeader {
    pub msgid: MessageId,
    pub count: u16,
    pub mac: u64
}

#[derive(Debug, Copy, Clone)]
pub struct ReqHeader {
    pub mac: u64
}

impl ReqHeader {
    fn as_bytes(&self) -> Vec<u8> {
        format!("{:016X}", self.mac).bytes().collect()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Ack {
    pub status: u16,
    pub mac: Option<u64>
}

impl Ack {
    /// Decode info response
    fn new(decoder: raw::RawDataConsumer) -> error::PlResult<Ack> {
        let (decoder, status) = try!(decoder.decode::<u16>());
        let (decoder, mac) = if decoder.get_remaining() > 0 {
            let (decoder, mac) = try!(decoder.decode::<u64>());
            (decoder, Some(mac))
        } else {
            (decoder, None)
        };
        try!(decoder.check_fully_consumed());

        Ok(Ack {
            status: status,
            mac: mac
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ResInitialize {
    pub unknown1: u8,
    pub is_online: bool,
    pub network_id: u64,
    pub short_id: u16,
    pub unknown2:  u8
}

impl ResInitialize {
    /// Decode initialization response
    fn new(decoder: raw::RawDataConsumer) -> error::PlResult<ResInitialize> {
        let (decoder, unknown1) = try!(decoder.decode::<u8>());
        let (decoder, is_online) = try!(decoder.decode::<u8>());
        let (decoder, network_id) = try!(decoder.decode::<u64>());
        let (decoder, short_id) = try!(decoder.decode::<u16>());
        let (decoder, unknown2) = try!(decoder.decode::<u8>());
        try!(decoder.check_fully_consumed());

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
pub struct DateTime {
    year: u8,
    months: u8,
    minutes: u16
}

impl DateTime {
    pub fn new(timestamp: Tm) -> DateTime{
        let utc = timestamp.to_utc();

        DateTime {
            year: (utc.tm_year - 100) as u8,
            months: (utc.tm_mon + 1) as u8,
            minutes: (((utc.tm_mday - 1) * 24 * 60) + (utc.tm_hour * 60) + utc.tm_min) as u16
        }
    }

    pub fn new_raw(year: u8, months: u8, minutes: u16) -> DateTime {
        DateTime {
            year: year,
            months: months,
            minutes: minutes
        }
    }

    pub fn to_tm(&self) -> Option<Tm> {
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
pub struct ResInfo {
    pub datetime: DateTime,
    pub last_logaddr: u32,
    pub relay_state: bool,
    pub hz: u8,
    pub hw_ver: String,
    pub fw_ver: Timespec,
    pub unknown: u8
}

impl ResInfo {
    /// Decode info response
    fn new(decoder: raw::RawDataConsumer) -> error::PlResult<ResInfo> {
        let (decoder, datetime) = try!(decoder.decode_datetime());
        let (decoder, last_logaddr) = try!(decoder.decode::<u32>());
        let (decoder, relay_state) = try!(decoder.decode::<u8>());
        let (decoder, hz) = try!(decoder.decode::<u8>());
        let (decoder, hw_ver) = try!(decoder.decode_string(12));
        let (decoder, fw_ver) = try!(decoder.decode::<u32>());
        let (decoder, unknown) = try!(decoder.decode::<u8>());
        try!(decoder.check_fully_consumed());

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
pub struct ReqSwitch{
    pub on: bool
}

impl ReqSwitch {
    fn as_bytes(&self) -> Vec<u8> {
        let on = if self.on {1} else {0};

        format!("{:02X}", on).bytes().collect()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ResCalibration {
    pub gain_a: f32,
    pub gain_b: f32,
    pub off_total: f32,
    pub off_noise: f32
}

impl ResCalibration {
    fn new(decoder: raw::RawDataConsumer) -> error::PlResult<ResCalibration> {
        let (decoder, gain_a) = try!(decoder.decode_f32());
        let (decoder, gain_b) = try!(decoder.decode_f32());
        let (decoder, off_total) = try!(decoder.decode_f32());
        let (decoder, off_noise) = try!(decoder.decode_f32());
        try!(decoder.check_fully_consumed());

        Ok(ResCalibration {
            gain_a: gain_a,
            gain_b: gain_b,
            off_total: off_total,
            off_noise: off_noise
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ReqPowerBuffer {
    pub logaddr: u32
}

impl ReqPowerBuffer {
    fn as_bytes(&self) -> Vec<u8> {
        let logaddr = pos2addr(self.logaddr);

        format!("{:08X}", logaddr).bytes().collect()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ResPowerBuffer {
    pub datetime1: DateTime,
    pub pulses1: Pulses,
    pub datetime2: DateTime,
    pub pulses2: Pulses,
    pub datetime3: DateTime,
    pub pulses3: Pulses,
    pub datetime4: DateTime,
    pub pulses4: Pulses,
    pub logaddr: u32,
}

impl ResPowerBuffer {
    fn new(decoder: raw::RawDataConsumer) -> error::PlResult<ResPowerBuffer> {
        let (decoder, datetime1) = try!(decoder.decode_datetime());
        let (decoder, pulses1) = try!(decoder.decode::<u32>());
        let (decoder, datetime2) = try!(decoder.decode_datetime());
        let (decoder, pulses2) = try!(decoder.decode::<u32>());
        let (decoder, datetime3) = try!(decoder.decode_datetime());
        let (decoder, pulses3) = try!(decoder.decode::<u32>());
        let (decoder, datetime4) = try!(decoder.decode_datetime());
        let (decoder, pulses4) = try!(decoder.decode::<u32>());
        let (decoder, logaddr) = try!(decoder.decode::<u32>());
        try!(decoder.check_fully_consumed());

        Ok(ResPowerBuffer {
            datetime1: datetime1,
            pulses1: Pulses::new(pulses1, 3600),
            datetime2: datetime2,
            pulses2: Pulses::new(pulses2, 3600),
            datetime3: datetime3,
            pulses3: Pulses::new(pulses3, 3600),
            datetime4: datetime4,
            pulses4: Pulses::new(pulses4, 3600),
            logaddr: addr2pos(logaddr)
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ResPowerUse {
    pub pulse_1s: Pulses,
    pub pulse_8s: Pulses,
    pub pulse_hour: Pulses,
    pub unknown1: u16,
    pub unknown2: u16,
    pub unknown3: u16,
}

impl ResPowerUse {
    fn new(decoder: raw::RawDataConsumer) -> error::PlResult<ResPowerUse> {
        let (decoder, pulse_1s) = try!(decoder.decode::<u16>());
        let (decoder, pulse_8s) = try!(decoder.decode::<u16>());
        let (decoder, pulse_hour) = try!(decoder.decode::<u32>());
        let (decoder, unknown1) = try!(decoder.decode::<u16>());
        let (decoder, unknown2) = try!(decoder.decode::<u16>());
        let (decoder, unknown3) = try!(decoder.decode::<u16>());
        try!(decoder.check_fully_consumed());

        Ok(ResPowerUse {
            pulse_1s: Pulses::new(pulse_1s as u32, 1),
            pulse_8s: Pulses::new(pulse_8s as u32, 8),
            pulse_hour: Pulses::new(pulse_hour, 3600),
            unknown1: unknown1,
            unknown2: unknown2,
            unknown3: unknown3,
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ResClockInfo {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub day_of_week: u8,
    pub unknown1: u8,
    pub unknown2: u16
}

impl ResClockInfo {
    fn new(decoder: raw::RawDataConsumer) -> error::PlResult<ResClockInfo> {
        let (decoder, hour) = try!(decoder.decode::<u8>());
        let (decoder, minute) = try!(decoder.decode::<u8>());
        let (decoder, second) = try!(decoder.decode::<u8>());
        let (decoder, day_of_week) = try!(decoder.decode::<u8>());
        let (decoder, unknown1) = try!(decoder.decode::<u8>());
        let (decoder, unknown2) = try!(decoder.decode::<u16>());
        try!(decoder.check_fully_consumed());

        Ok(ResClockInfo {
            hour: hour,
            minute: minute,
            second: second,
            day_of_week: day_of_week,
            unknown1: unknown1,
            unknown2: unknown2
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ReqClockSet {
    pub datetime: DateTime,
    pub logaddr: Option<u32>,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub day_of_week: u8,
}

impl ReqClockSet {
    pub fn new_from_tm(tm: Tm) -> ReqClockSet {
        let utc = tm.to_utc();
        let day_of_week = match tm.tm_wday {
            n @ 1...6 => n as u8,
            0 => 7 as u8,
            _ => unreachable!()
        };

        ReqClockSet {
            datetime: DateTime::new(utc),
            logaddr: None,
            hour: utc.tm_hour as u8,
            minute: utc.tm_min as u8,
            second: utc.tm_sec as u8,
            day_of_week: day_of_week,
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
pub enum MessageId {
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
pub enum Message {
    Ack(ResHeader, Ack),
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
    pub fn to_payload(&self) -> error::PlResult<Vec<u8>> {
        let mut vec = vec![];

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
            _ => Err(error::PlError::Protocol)
        }
    }

    /// Convert given bunch of bytes to interpretable message
    pub fn from_payload(payload: &[u8]) -> error::PlResult<Message> {
        let decoder = raw::RawDataConsumer::new(payload);

        let (decoder, msg_id) = try!(decoder.decode::<u16>());
        let (decoder, counter) = try!(decoder.decode::<u16>());
        let msg_id = MessageId::new(msg_id);

        let (decoder, mac) = if msg_id != MessageId::Ack {
            try!(decoder.decode::<u64>())
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
            MessageId::Ack =>
                Ok(Message::Ack(header, try!(Ack::new(decoder)))),
            _ =>
                Err(error::PlError::Protocol)
        }
    }

    pub fn to_message_id(&self) -> MessageId {
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
