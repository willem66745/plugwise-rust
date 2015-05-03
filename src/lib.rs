
extern crate crc16;
extern crate time;
extern crate serial;

mod stub;
mod protocol;

use std::io::prelude::*;
use std::io;
use time::Duration;
use serial::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::BTreeMap;

pub use protocol::ProtocolSnoop;

struct PlugwiseInner<'a, I> {
    protocol: Rc<RefCell<protocol::Protocol<'a, I>>>
}

struct CircleInner<'a, I> {
    protocol: Rc<RefCell<protocol::Protocol<'a, I>>>,
    mac: u64,
    calibration_data: protocol::ResCalibration
}

impl<'a, I: Read+Write+'a> PlugwiseInner<'a, I> {
    fn initialize(port: I) -> io::Result<PlugwiseInner<'a, I>> {
        let plugwise = PlugwiseInner {
            protocol: Rc::new(RefCell::new(protocol::Protocol::new(port)))
        };

        let result = try!(plugwise.protocol.borrow_mut().initialize());

        if !result.is_online {
            return Err(io::Error::new(io::ErrorKind::Other, "not online"));
        }

        Ok(plugwise)
    }
}

pub trait Plugwise<'a> {
    fn create_circle(&self, mac: u64) -> io::Result<Box<Circle + 'a>>;
    fn set_snoop(&self, snoop: ProtocolSnoop<'a>);
}

pub trait Circle {
    fn switch_on(&self) -> io::Result<()>;
    fn switch_off(&self) -> io::Result<()>;
    fn is_switched_on(&self) -> io::Result<bool>;
    fn get_actual_watt_usage(&self) -> io::Result<f64>;
    fn get_clock(&self) -> io::Result<time::Tm>;
    fn set_clock(&self, tm: time::Tm) -> io::Result<()>;
    fn get_power_buffer(&self, max_entries: Option<u32>) -> io::Result<BTreeMap<time::Timespec, f64>>;
}

impl<'a, I:Read+Write+'a> Plugwise<'a> for PlugwiseInner<'a, I> {
    fn create_circle(&self, mac: u64) -> io::Result<Box<Circle+ 'a>> {
        let calibration_data = try!(self.protocol.borrow_mut().calibrate(mac));
        Ok(Box::new(CircleInner {
            protocol: self.protocol.clone(),
            mac: mac,
            calibration_data: calibration_data
        }))
    }

    fn set_snoop(&self, snoop: ProtocolSnoop<'a>) {
        self.protocol.borrow_mut().set_snoop(snoop);
    }
}

impl<'a, I:Read+Write+'a> Circle for CircleInner<'a, I> {
    fn switch_on(&self) -> io::Result<()> {
        try!(self.protocol.borrow_mut().switch(self.mac, true));
        Ok(())
    }

    fn switch_off(&self) -> io::Result<()> {
        try!(self.protocol.borrow_mut().switch(self.mac, false));
        Ok(())
    }

    fn is_switched_on(&self) -> io::Result<bool> {
        let info = try!(self.protocol.borrow_mut().get_info(self.mac));
        Ok(info.relay_state)
    }

    fn get_actual_watt_usage(&self) -> io::Result<f64> {
        let power_usage = try!(self.protocol.borrow_mut().get_power_usage(self.mac));
        Ok(power_usage.pulse_8s.to_watts(self.calibration_data))
    }

    fn get_clock(&self) -> io::Result<time::Tm> {
        let info = try!(self.protocol.borrow_mut().get_info(self.mac));
        let clock = try!(self.protocol.borrow_mut().get_clock_info(self.mac));

        let mut tm = match info.datetime.to_tm() {
            Some(tm) => tm,
            None => return Err(io::Error::new(io::ErrorKind::Other, "circle returns an invalid timestamp"))
        };
        tm.tm_sec = clock.second as i32;
        tm.tm_min = clock.minute as i32;
        tm.tm_hour = clock.hour as i32;
        tm.tm_wday = (clock.day_of_week % 7) as i32;
        Ok(tm)
    }

    fn set_clock(&self, tm: time::Tm) -> io::Result<()> {
        let clock_set = protocol::ReqClockSet::new_from_tm(tm);
        try!(self.protocol.borrow_mut().set_clock(self.mac, clock_set));
        Ok(())
    }


    fn get_power_buffer(&self, max_entries: Option<u32>) -> io::Result<BTreeMap<time::Timespec, f64>> {
        let mut result = BTreeMap::<time::Timespec, f64>::new();
        let info = try!(self.protocol.borrow_mut().get_info(self.mac));
        let start = match max_entries {
            None => 0,
            Some(n) => {
                let n_of_calls = n / 4; // each power buffer request retrieves 4 power usage statics
                if info.last_logaddr > n_of_calls {
                    info.last_logaddr - n_of_calls
                } else {
                    0
                }
            }
        };

        for index in (start..(info.last_logaddr + 1)) {
            let buffer = try!(self.protocol.borrow_mut().get_power_buffer(self.mac, index));

            self.get_power_buffer_helper(&mut result, &buffer.datetime1, &buffer.pulses1);
            self.get_power_buffer_helper(&mut result, &buffer.datetime2, &buffer.pulses2);
            self.get_power_buffer_helper(&mut result, &buffer.datetime3, &buffer.pulses3);
            self.get_power_buffer_helper(&mut result, &buffer.datetime4, &buffer.pulses4);
        }

        Ok(result)
    }
}

impl <'a, I:Read+Write+'a>  CircleInner<'a, I> {
    fn get_power_buffer_helper(&self,
                               map:&mut BTreeMap<time::Timespec, f64>,
                               datetime: &protocol::DateTime,
                               pulses: &protocol::Pulses) {
        if let Some(tm) = datetime.to_tm() {
            let _ = map.insert(tm.to_timespec(), pulses.to_kwh(self.calibration_data));
        }
    }
}

pub fn plugwise_device<'a>(device: &str) -> io::Result<Box<Plugwise<'a>+ 'a>> {
    let mut port = try!(serial::open(device));
    try!(port.configure(|settings| {
        settings.set_baud_rate(serial::Baud115200);
        settings.set_char_size(serial::Bits8);
        settings.set_parity(serial::ParityNone);
        settings.set_stop_bits(serial::Stop1);
    }));

    port.set_timeout(Duration::milliseconds(1000));
    let plugwise = try!(PlugwiseInner::initialize(port));

    Ok(Box::new(plugwise))
}

pub fn plugwise_simulator<'a>() -> io::Result<Box<Plugwise<'a>+ 'a>> {
    let port = stub::Stub::new();

    let plugwise = try!(PlugwiseInner::initialize(port));

    Ok(Box::new(plugwise))
}

#[test]
fn smoke_external_stub() {
    let stub = plugwise_simulator().unwrap();
    let circle = stub.create_circle(0x01234567890ABCDEF).unwrap();
    circle.switch_on().unwrap();
    assert_eq!(circle.is_switched_on().unwrap(), true);
    circle.switch_off().unwrap();
    assert_eq!(circle.is_switched_on().unwrap(), false);
    circle.get_actual_watt_usage().unwrap();
    let tm = circle.get_clock().unwrap();
    circle.set_clock(tm).unwrap();
    circle.get_power_buffer(None);
}
