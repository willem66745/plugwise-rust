
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

pub struct ActualPowerUsage {
    pub now: f64, // kWh
    pub last_hour: f64 // kWh
}

pub trait Circle {
    fn switch_on(&self) -> io::Result<()>;
    fn switch_off(&self) -> io::Result<()>;
    fn is_switched_on(&self) -> io::Result<bool>;
    fn get_actual_power_usage(&self) -> io::Result<ActualPowerUsage>;
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

    fn get_actual_power_usage(&self) -> io::Result<ActualPowerUsage> {
        let power_usage = try!(self.protocol.borrow_mut().get_power_usage(self.mac));
        Ok(ActualPowerUsage {
            now: power_usage.pulse_8s.to_kwh(self.calibration_data),
            last_hour: power_usage.pulse_hour.to_kwh(self.calibration_data)
        })
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
