
extern crate plugwise;
extern crate toml;
extern crate time;

use std::io;
use std::io::prelude::*;
use std::env::home_dir;
use std::fs::File;

use time::Duration;

use plugwise::ProtocolSnoop;
use plugwise::Device;
use plugwise::plugwise;

fn main() {
    let mut path = home_dir().unwrap(); // XXX
    path.push("plugwise.toml");
    let mut file = File::open(&path).unwrap();
    let mut config = String::new();
    file.read_to_string(&mut config).unwrap();
    let config = toml::Parser::new(&config).parse().unwrap(); // XXX

    let mut debug = io::stdout();
    //let plugwise = plugwise(Device::Simulator).unwrap();
    let plugwise = plugwise(Device::SerialExt("/dev/ttyUSB0",
                                              Duration::milliseconds(1000),
                                              3,
                                              ProtocolSnoop::Debug(&mut debug))).unwrap();

    let week = time::Duration::weeks(1);

    for (_, item) in config {
        if let Some(mac) = item.as_table().unwrap().get("mac") { // XXX
            let mac = mac.as_str().unwrap(); // XXX
            let mac = u64::from_str_radix(mac, 16).unwrap(); // XXX

            let circle = plugwise.create_circle(mac).unwrap();
            //circle.switch_on().unwrap();
            //println!("{}", circle.is_switched_on().unwrap());
            //circle.switch_off().unwrap();
            //println!("{}", circle.is_switched_on().unwrap());
            let power = circle.get_actual_watt_usage().unwrap();
            println!("Plug: {:08X}", mac);
            println!("Actual usage: {} W", power);
            let buffer = circle.get_power_buffer(Some(week.num_hours() as u32)).unwrap();
            if let Some(last_timestamp) = buffer.keys().last() {
                let kws = buffer.iter()
                                .filter(|&(&k, _)| (*last_timestamp - k) < week)
                                .fold(0.0, |acc, (_, &v)| acc + v);
                println!("Power usage last week: {} kWh", kws);
            }
        }
    }
}
