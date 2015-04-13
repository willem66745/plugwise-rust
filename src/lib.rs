pub mod sim;
pub mod real;

pub trait System {
    fn register_plug(&mut self, id: &str) -> Box<Plug>;
}

pub trait Plug {
    fn get_id(&self) -> &str;
    fn is_switched_on(&self) -> bool;
    fn switch_on(&mut self);
    fn switch_off(&mut self);
}

