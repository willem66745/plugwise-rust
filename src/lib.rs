extern crate crc16;
extern crate time;

pub mod sim; // XXX: remove
pub mod real; // XXX: remove
pub mod stub;
pub mod protocol; // XXX: remove pub after abstractions are created

// XXX: remove
pub trait System {
    fn register_plug(&mut self, id: &str) -> Box<Plug>;
}

// XXX: remove
pub trait Plug {
    fn get_id(&self) -> &str;
    fn is_switched_on(&self) -> bool;
    fn switch_on(&mut self);
    fn switch_off(&mut self);
}

