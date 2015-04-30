
extern crate plugwise;

use plugwise::*;

fn main() {
    let mut plug1;
    let mut plug2;
    let mut plug3;

    {
        let mut plugwise = sim::Plugwise::new("/dev/usbserial");

        plug1 = plugwise.register_plug("000D6F0000000001");
        plug2 = plugwise.register_plug("000D6F0000000002");
        plug3 = plugwise.register_plug("000D6F0000000003");
    }

    println!("{}={}", plug1.get_id(), plug1.is_switched_on());
    println!("{}={}", plug2.get_id(), plug2.is_switched_on());
    println!("{}={}", plug3.get_id(), plug3.is_switched_on());
    plug1.switch_on();
    plug2.switch_on();
    plug3.switch_on();
    println!("{}={}", plug1.get_id(), plug1.is_switched_on());
    println!("{}={}", plug2.get_id(), plug2.is_switched_on());
    println!("{}={}", plug3.get_id(), plug3.is_switched_on());
    plug1.switch_off();
    plug2.switch_off();
    plug3.switch_off();
    println!("{}={}", plug1.get_id(), plug1.is_switched_on());
    println!("{}={}", plug2.get_id(), plug2.is_switched_on());
    println!("{}={}", plug3.get_id(), plug3.is_switched_on());
}
