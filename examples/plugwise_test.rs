//extern crate serial;
extern crate time;
extern crate crc16;
extern crate toml;
extern crate plugwise;

use std::io;
use std::io::prelude::*;
//use serial::prelude::*;
//use time::Duration;
use crc16::*;
use std::fs::File;
use std::env::home_dir;
use plugwise::stub;
use plugwise::protocol::*;

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
