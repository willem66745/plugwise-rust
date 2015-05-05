//! This crate implements a part of the Plugwise Circle and Plugwise Circle+ protocol (tested against
//! 2010 firmware). It still requires the official tooling to configure and to link the plugs. This
//! library supports the following operations:
//!
//! * switch a Circle on or off;
//! * retrieve the relay status of a Circle;
//! * actual power usage of a Circle (in Watts);
//! * power usage over time (retrieved per hour in kWh);
//! * set clock of a Circle;
//! * get actual clock of a Circle.
//!
//! This library is inspired on a
//! [Python implemention](https://bitbucket.org/hadara/python-plugwise/wiki/Home) which was based
//! on the analysis of the protocol by
//! [Maarten Damen](http://www.maartendamen.com/category/plugwise-unleashed/).
//!
//! This crate is tested against Linux, but since this crate is based on
//! [serial-rs](../serial/index.html) crate, it is expected this crate also works on Windows and
//! Mac OS X.
//!
//! Enable the relay of a Circle:
//!
//! ```ignore
//! extern crate plugwise;
//!
//! // instantiate a simulation version of Plugwise
//! let stub = plugwise::plugwise(plugwise::Device::Serial("/dev/ttyUSB0")).unwrap();
//! // create a Circle (simulation allows any MAC to be used)
//! let circle = stub.create_circle(0x01234567890ABCDEF).unwrap();
//! // switch the Circle on
//! circle.switch_on().unwrap();
//! ```

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

    fn set_snoop(&self, snoop: ProtocolSnoop<'a>) {
        self.protocol.borrow_mut().set_snoop(snoop);
    }

    fn set_retries(&self, retries: u8) {
        self.protocol.borrow_mut().set_retries(retries);
    }
}

/// A abstract representation of the Plugwise USB stick.
pub trait Plugwise<'a> {
    /// Register a Circle (a wall outlet switch) and returns a abstract representation of the
    /// Circle.
    fn create_circle(&self, mac: u64) -> io::Result<Box<Circle + 'a>>;
}

/// A abstract representation of the Plugwise Circle/Circle+.
pub trait Circle {
    /// Switch the relay of Circle on.
    fn switch_on(&self) -> io::Result<()>;
    /// Switch the relay of Circle off.
    fn switch_off(&self) -> io::Result<()>;
    /// Retrieve the relay status of the Circle.
    fn is_switched_on(&self) -> io::Result<bool>;
    /// Get actual power usage of the Circle in Watts (sampled over the last 8 seconds).
    fn get_actual_watt_usage(&self) -> io::Result<f64>;
    /// Get the actual clock state of the Circle (in UTC).
    fn get_clock(&self) -> io::Result<time::Tm>;
    /// Set the clock state of the Circle.
    fn set_clock(&self, tm: time::Tm) -> io::Result<()>;
    /// Retrieve a map of power usages over time. To retrieve only the last logged items specify
    /// the number of elements to retrieve in `max_entries`. Each entry contains the power usage of
    /// one hour.
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


    fn get_power_buffer(&self,
                        max_entries: Option<u32>)
                        -> io::Result<BTreeMap<time::Timespec, f64>> {
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
                               map: &mut BTreeMap<time::Timespec, f64>,
                               datetime: &protocol::DateTime,
                               pulses: &protocol::Pulses) {
        if let Some(tm) = datetime.to_tm() {
            let _ = map.insert(tm.to_timespec(), pulses.to_kwh(self.calibration_data));
        }
    }
}

/// Create a instance to communicate with the Plugwise USB stick and Circle/Circle+ devices.
fn plugwise_device<'a>(port: &str,
                       timeout: Duration,
                       retries: u8,
                       snoop: ProtocolSnoop<'a>) -> io::Result<Box<Plugwise<'a>+ 'a>> {
    let mut port = try!(serial::open(port));
    try!(port.configure(|settings| {
        settings.set_baud_rate(serial::Baud115200);
        settings.set_char_size(serial::Bits8);
        settings.set_parity(serial::ParityNone);
        settings.set_stop_bits(serial::Stop1);
    }));

    port.set_timeout(timeout);
    let plugwise = try!(PlugwiseInner::initialize(port));
    plugwise.set_snoop(snoop);
    plugwise.set_retries(retries);

    Ok(Box::new(plugwise))
}

/// Create a simulation instance for integration or test purposes to use this library without the
/// need of the real hardware.
fn plugwise_simulator<'a>() -> io::Result<Box<Plugwise<'a>+ 'a>> {
    let port = stub::Stub::new();

    let plugwise = try!(PlugwiseInner::initialize(port));

    Ok(Box::new(plugwise))
}

/// Specify which kind of Plugwise device to use
pub enum Device<'a> {
    /// Create a link to the Plugwise USB stick to communicate with the Circle/Circle+ wall
    /// outlets. The reference to the hardware device (i.e. `/dev/ttyUSB0`) must be provided.
    Serial(&'a str),
    /// Simular to `serial` but with extra settings:
    /// Timeout in milliseconds;
    /// Number of attempts to retry communication;
    /// Tracing settings (including a reference to a `io::Write` instance to log the
    /// communication)
    SerialExt(&'a str, time::Duration, u8, ProtocolSnoop<'a>),
    /// Create a simulation instance for development, testing and integration purposes
    Simulator,
}

/// Create instance to communicate against a (simulator) Plugwise USB stick and the associated
/// Circle/Circle+ devices.
///
/// Instantiate a link to a Plugwise device:
///
/// ```ignore
/// extern crate plugwise;
///
/// // instantiate a simulation version of Plugwise
/// let stub = plugwise::plugwise(plugwise::Device::Serial("/dev/ttyUSB0")).unwrap();
/// // create a Circle (simulation allows any MAC to be used)
/// let circle = stub.create_circle(0x01234567890ABCDEF).unwrap();
/// // switch the Circle on
/// circle.switch_on().unwrap();
/// ```
///
/// Instantiate a simulation version:
///
/// ```
/// extern crate plugwise;
///
/// // instantiate a simulation version of Plugwise
/// let stub = plugwise::plugwise(plugwise::Device::Simulator).unwrap();
/// // create a Circle (simulation allows any MAC to be used)
/// let circle = stub.create_circle(0x01234567890ABCDEF).unwrap();
/// // switch the Circle on
/// circle.switch_on().unwrap();
/// ```
pub fn plugwise<'a>(device: Device<'a>) -> io::Result<Box<Plugwise<'a>+ 'a>> {
    match device {
        Device::Simulator => {
            plugwise_simulator()
        },
        Device::Serial(port) => {
            plugwise_device(port, Duration::milliseconds(1000), 3, ProtocolSnoop::Nothing)
        },
        Device::SerialExt(port, timeout, retries, snoop) => {
            let device = try!(plugwise_device(port, timeout, retries, snoop));

            Ok(device)
        },
    }
}

#[test]
fn smoke_external_stub() {
    let stub = plugwise(Device::Simulator).unwrap();
    let circle = stub.create_circle(0x01234567890ABCDEF).unwrap();
    circle.switch_on().unwrap();
    assert_eq!(circle.is_switched_on().unwrap(), true);
    circle.switch_off().unwrap();
    assert_eq!(circle.is_switched_on().unwrap(), false);
    circle.get_actual_watt_usage().unwrap();
    let tm = circle.get_clock().unwrap();
    circle.set_clock(tm).unwrap();
    circle.get_power_buffer(None).unwrap();
}
