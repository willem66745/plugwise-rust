use super::{Plug, System};
use std::rc::Rc;

// main container

pub struct Plugwise {
    inner: Rc<SimProtocol>,
}

impl Plugwise {
    pub fn new(_: &str) -> Plugwise {
        Plugwise {
            inner: Rc::new(SimProtocol),
        }
    }
}

impl System for Plugwise {
    fn register_plug(&mut self, id: &str) -> Box<Plug> {
        let switch = SimPlug::new(id, self.inner.clone());

        Box::new(switch)
    }
}

// switch

struct SimPlug {
    id: String,
    switch_enabled: bool,
    inner: Rc<SimProtocol>,
}

impl SimPlug {
    fn new(id: &str, inner:Rc<SimProtocol>) -> SimPlug {
        SimPlug {
            id: id.to_string(),
            switch_enabled: false,
            inner: inner,
        }
    }
}

impl Plug for SimPlug {
    fn is_switched_on(&self) -> bool {
        self.switch_enabled
    }

    fn switch_on(&mut self) {
        self.inner.do_action(&self.id, "on");
        self.switch_enabled = true;
    }

    fn switch_off(&mut self) {
        self.inner.do_action(&self.id, "off");
        self.switch_enabled = false;
    }

    fn get_id(&self) -> &str {
        &self.id
    }
}

// inner / logical part

struct SimProtocol;

impl SimProtocol {
    fn do_action(&self, id: &str, action: &str) {
        println!("{} {}", id, action);
    }
}
