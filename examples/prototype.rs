
extern crate plugwise;

use std::io;

use plugwise::ProtocolSnoop;

fn main() {
    let mut debug = io::stdout();

    //let plugwise = plugwise::plugwise_device("/dev/ttyUSB0").unwrap();
    let plugwise = plugwise::plugwise_simulator().unwrap();
    plugwise.set_snoop(ProtocolSnoop::Debug(&mut debug));
    let circle = plugwise.create_circle(0x0123456789ABCDEF).unwrap();
    circle.switch_on().unwrap();
    println!("{}", circle.is_switched_on().unwrap());
    circle.switch_off().unwrap();
    println!("{}", circle.is_switched_on().unwrap());
}
