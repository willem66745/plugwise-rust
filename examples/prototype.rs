
extern crate plugwise;
extern crate toml;

use std::io;
use std::io::prelude::*;
use std::env::home_dir;
use std::fs::File;

use plugwise::ProtocolSnoop;

fn main() {
    let mut path = home_dir().unwrap(); // XXX
    path.push("plugwise.toml");
    let mut file = File::open(&path).unwrap();
    let mut config = String::new();
    file.read_to_string(&mut config).unwrap();
    let config = toml::Parser::new(&config).parse().unwrap(); // XXX

    let mut debug = io::stdout();
    //let plugwise = plugwise::plugwise_device("/dev/ttyUSB0").unwrap();
    let plugwise = plugwise::plugwise_simulator().unwrap();
    plugwise.set_snoop(ProtocolSnoop::Debug(&mut debug));

    for (_, item) in config {
        if let Some(mac) = item.as_table().unwrap().get("mac") { // XXX
            let mac = mac.as_str().unwrap(); // XXX
            let mac = u64::from_str_radix(mac, 16).unwrap(); // XXX

            let circle = plugwise.create_circle(mac).unwrap();
            circle.switch_on().unwrap();
            println!("{}", circle.is_switched_on().unwrap());
            circle.switch_off().unwrap();
            println!("{}", circle.is_switched_on().unwrap());
        }
    }
}
