
extern crate time;
extern crate serial;
extern crate plugwise;

use std::io::prelude::*;
use plugwise::protocol::*;
use plugwise::stub;
use std::io;
use time::Duration;
use serial::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;

struct PlugwiseInner<'a, I> {
    protocol: Rc<RefCell<Protocol<'a, I>>>
}

struct CircleInner<'a, I> {
    protocol: Rc<RefCell<Protocol<'a, I>>>,
    mac: u64
}

impl<'a, I: Read+Write+'a> PlugwiseInner<'a, I> {
    fn initialize(port: I) -> io::Result<PlugwiseInner<'a, I>> {
        let plugwise = PlugwiseInner {
            protocol: Rc::new(RefCell::new(Protocol::new(port)))
        };

        let result = try!(plugwise.protocol.borrow_mut().initialize());

        if !result.is_online {
            return Err(io::Error::new(io::ErrorKind::Other, "not online"));
        }

        Ok(plugwise)
    }
}

trait Plugwise<'a> {
    fn create_circle(&self, mac: u64) -> Box<Circle + 'a>;
    fn set_snoop(&self, snoop: ProtocolSnoop<'a>);
}

trait Circle {
    fn switch_on(&self) -> io::Result<()>;
    fn switch_off(&self) -> io::Result<()>;
}

impl<'a, I:Read+Write+'a> Plugwise<'a> for PlugwiseInner<'a, I> {
    fn create_circle(&self, mac: u64) -> Box<Circle+ 'a> {
        //Box::new(CircleInner::new(self.protocol.clone(), mac))
        Box::new(CircleInner {
            protocol: self.protocol.clone(),
            mac: mac
        })
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
}

fn plugwise_device<'a>(device: &str) -> io::Result<Box<Plugwise<'a>+ 'a>> {
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

fn plugwise_simulator<'a>() -> io::Result<Box<Plugwise<'a>+ 'a>> {
    let port = stub::Stub::new();

    let plugwise = try!(PlugwiseInner::initialize(port));

    Ok(Box::new(plugwise))
}

fn main() {
    let mut debug = io::stdout();

    //let plugwise = plugwise_device("/dev/ttyUSB0").unwrap();
    let plugwise = plugwise_simulator().unwrap();
    plugwise.set_snoop(ProtocolSnoop::Debug(&mut debug));
    let circle = plugwise.create_circle(0x0123456789ABCDEF);
    circle.switch_on().unwrap();
    circle.switch_off().unwrap();
}
