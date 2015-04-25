use std::io;
use std::io::prelude::*;
use std::cmp;
use std::str;
use std::collections::BTreeMap;
use crc16::*;

// NOTE: keep this component free of dependencies to other modules within this
//       crate.

const HEADER: [u8; 4] = [5, 5, 3, 3];
const FOOTER: [u8; 2] = [13, 10];

// Simulation state
#[derive(Debug, Copy, Clone)]
enum PlugState {
    Off,
    On,
}

/// Replacement for hardware for qualification and high-level integration
/// purposes.
///
/// It only represents "perfect world" behavior, and only keps switch states
/// but no power levels, etc...
pub struct Stub {
    input: Vec<u8>,
    responses: Vec<Vec<u8>>,
    output: Vec<u8>,
    plug: BTreeMap<u64, PlugState>,
}

impl Stub {
    pub fn new() -> Stub {
        Stub {
            input: vec![],
            responses: vec![],
            output: vec![],
            plug: BTreeMap::<u64, PlugState>::new(),
        }
    }

    fn from_hex_buffer(buf: &[u8]) -> u64 {
        // it can panic when invalid buffers or invalid values are provided, at
        // the other hand, as test facility, this might even be considered as
        // intended behavior.
        u64::from_str_radix(str::from_utf8(buf).unwrap(), 16).unwrap()
    }

    fn handle_incoming(&mut self, buf: &[u8]) -> io::Result<()> {
        let (command, payload) = buf.split_at(4);
        let (mac, payload) = if command != b"000A" {
            let (mac, payload) = payload.split_at(16);
            (Stub::from_hex_buffer(mac), payload)
        } else {
            (0, payload)
        };
        let macbuf = format!("{:016X}", mac).into_bytes();
        if command == b"0017" {
            // remember switch state
            let (switch, _) = payload.split_at(2);
            let switch = Stub::from_hex_buffer(switch);
            let switch = if switch == 0 {
                PlugState::Off
            } else {
                PlugState::On
            };
            self.plug.insert(mac, switch);
        }

        match command {
            b"000A" => self.responses.push(b"00110000000000000000000001010000000000000000000000".to_vec()),
            b"0016"|b"0017" => {
                let mut ack = vec![];
                ack.extend(b"000000000000".iter().cloned());
                ack.extend(macbuf);
                self.responses.push(ack);
            },
            b"0023" => {
                let state = self.plug.get(&mac);
                let state = match state {
                    None |
                    Some(&PlugState::Off) => 0,
                    Some(&PlugState::On) => 1
                };
                let mut ack = vec![];
                ack.extend(b"00240000".iter().cloned());
                ack.extend(macbuf);
                ack.extend(format!("0F0489B800048398{:02X}856539070140234E0844C202", state).into_bytes());
                self.responses.push(ack);
            },
            b"0026" => {
                let mut ack = vec![];
                ack.extend(b"00270000".iter().cloned());
                ack.extend(macbuf);
                ack.extend(b"00000000000000000000000000000000".iter().cloned());
                self.responses.push(ack);
            },
            b"0048" => {
                let mut ack = vec![];
                ack.extend(b"00490000".iter().cloned());
                ack.extend(macbuf);
                ack.extend(b"0D094D1C0000007B0D094D58000000760D094D94000000710D094DD00000003100044000".iter().cloned());
                self.responses.push(ack);
            },
            b"0012" => {
                let mut ack = vec![];
                ack.extend(b"00130000".iter().cloned());
                ack.extend(macbuf);
                ack.extend(b"0000000000000000000000000000".iter().cloned());
                self.responses.push(ack);
            },
            b"003E" => {
                let mut ack = vec![];
                ack.extend(b"003F0000".iter().cloned());
                ack.extend(macbuf);
                ack.extend(b"0B243A0601457A".iter().cloned());
                self.responses.push(ack);
            },
            _ => return Err(io::Error::new(io::ErrorKind::Other, "unsupported"))
        }

        Ok(())
    }
}

impl io::Read for Stub {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.output.len() == 0 {
            if self.responses.len() == 0 {
                return Err(io::Error::new(io::ErrorKind::Other, "no response pending"));
            }

            let new_response = self.responses.remove(0);
            let crc = format!("{:04X}", State::<XMODEM>::calculate(&new_response)).into_bytes();
            self.output.extend(HEADER.iter().cloned());
            self.output.extend(new_response);
            self.output.extend(crc);
            self.output.extend(FOOTER.iter().cloned());
        }

        let size = cmp::min(buf.len(), self.output.len());

        for i in (0..size) {
            buf[i] = self.output.remove(0);
        }

        Ok((size))
    }
}

impl io::Write for Stub {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.input.extend(buf.iter().cloned());
        // test whether a complete message has been received
        if let Some(pos) = self.input.iter().position(|x| *x==b'\r') {
            let input = self.input.clone();
            let (buf, _) = input.split_at(pos);

            // try to find the begin of the payload
            if let Some(rpos) = buf.iter().rposition(|x| *x==3) {
                let (_, buf) = buf.split_at(rpos + 1);

                try!(self.handle_incoming(buf));

                // no other way to deque a specific amount of data (or would
                // `self.input.drain().take(pos + 1)` a better approach in the
                // future?
                for _ in (0..pos + 1) {
                    let _ = self.input.remove(0);
                }
            }
        }
        Ok((buf.len()))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
