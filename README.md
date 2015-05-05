# plugwise-rust

This crate implements a part of the Plugwise Circle and Plugwise Circle+ protocol (tested against
2010 firmware). It still requires the official tooling to configure and to link the plugs. This
library supports the following operations:
                                                                                                  
* switch a Circle on or off;
* retrieve the relay status of a Circle;
* actual power usage of a Circle (in Watts);
* power usage over time (retrieved per hour in kWh);
* set clock of a Circle;
* get actual clock of a Circle.
                                                                                                  
This library is inspired on a
[Python implemention](https://bitbucket.org/hadara/python-plugwise/wiki/Home) which was based
on the analysis of the protocol by
[Maarten Damen](http://www.maartendamen.com/category/plugwise-unleashed/).
                                                                                                  
This crate is tested against Linux, but since this crate is based on
[serial-rs](https://github.com/dcuddeback/serial-rs) crate, it is expected this crate also works
on Windows and Mac OS X.

```rust
extern crate plugwise;

fn main() {
    // Connect to a plugwise device
    let serial = plugwise::plugwise(plugwise::Device::Serial("/dev/ttyUSB0")).unwrap();
    // create a Circle (replace MAC address)
    let circle = serial.create_circle(0x01234567890ABCDEF).unwrap();
    // switch the Circle on
    circle.switch_on().unwrap();
}
```
