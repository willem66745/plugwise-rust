extern crate serial;
extern crate time;
extern crate crc16;

use std::io;
use std::io::prelude::*;
use serial::prelude::*;
use time::Duration;
use crc16::*;

const HEADER: [u8; 4] = [5, 5, 3, 3];
const FOOTER: [u8; 2] = [13, 10];
const EOM: u8 = 10;
const CRC_SIZE: usize = 4;

fn main() {
    run().unwrap();
}

struct PlugwiseRawData<'a> {
    buf: &'a[u8],
}

impl<'a> PlugwiseRawData<'a> {
    fn new(buf: &'a[u8]) -> PlugwiseRawData<'a> {
        PlugwiseRawData {
            buf: buf,
        }
    }

    fn consume(&self, size: usize) -> io::Result<(&'a[u8], PlugwiseRawData)> {
        if (self.buf.len()) < size {
            return Err(io::Error::new(io::ErrorKind::Other, "data missing in received message"));
        }

        let (value, remainder) = self.buf.split_at(size);

        Ok((value, PlugwiseRawData {
            buf: remainder,
        }))
    }

    fn decode_u8(&self) -> io::Result<(PlugwiseRawData, u8)> {
        let (buf, result) = try!(self.consume(2));
        let mut value = 0;

        for byte in buf {
            value = value << 4 | match *byte {
                b'0' => 0, b'1' => 1, b'2' => 2,  b'3' => 3,  b'4' => 4,  b'5' => 5,  b'6' => 6,  b'7' => 7,
                b'8' => 8, b'9' => 9, b'A' => 10, b'B' => 11, b'C' => 12, b'D' => 13, b'E' => 14, b'F' => 15,
                _ => 0
            };
        }

        Ok((result, value))
    }

    fn decode_u16(&self) -> io::Result<(PlugwiseRawData, u16)> {
        let (buf, result) = try!(self.consume(4));
        let mut value = 0;

        for byte in buf {
            value = value << 4 | match *byte {
                b'0' => 0, b'1' => 1, b'2' => 2,  b'3' => 3,  b'4' => 4,  b'5' => 5,  b'6' => 6,  b'7' => 7,
                b'8' => 8, b'9' => 9, b'A' => 10, b'B' => 11, b'C' => 12, b'D' => 13, b'E' => 14, b'F' => 15,
                _ => 0
            };
        }

        Ok((result, value))
    }

    fn decode_u32(&self) -> io::Result<(PlugwiseRawData, u32)> {
        let (buf, result) = try!(self.consume(8));
        let mut value = 0;

        for byte in buf {
            value = value << 4 | match *byte {
                b'0' => 0, b'1' => 1, b'2' => 2,  b'3' => 3,  b'4' => 4,  b'5' => 5,  b'6' => 6,  b'7' => 7,
                b'8' => 8, b'9' => 9, b'A' => 10, b'B' => 11, b'C' => 12, b'D' => 13, b'E' => 14, b'F' => 15,
                _ => 0
            };
        }

        Ok((result, value))
    }

    fn decode_string(&self, size: usize) -> io::Result<(PlugwiseRawData, &'a str)> {
        let (buf, result) = try!(self.consume(size));

        match std::str::from_utf8(buf) {
            Ok(text) => Ok((result, text)),
            Err(err) => Err(io::Error::new(io::ErrorKind::Other, err))
        }
    }

    fn remainder(&self) -> io::Result<&'a str> {
        match std::str::from_utf8(self.buf) {
            Ok(value) => Ok(value),
            Err(err) => Err(io::Error::new(io::ErrorKind::Other, err))
        }
    }
}

struct PlugwiseProtocol<R> {
    reader: io::BufReader<R>
}

impl<R: Read + Write> PlugwiseProtocol<R> {
    /// Wrap IO entity for Plugwise protocol handling
    fn new(port: R) -> PlugwiseProtocol<R> {
        PlugwiseProtocol {
            reader: io::BufReader::with_capacity(1000, port)
        }
    }

    /// Send payload
    fn send_message_raw(&mut self, payload: &[u8]) -> io::Result<()> {
        let crc = format!("{:04X}", State::<XMODEM>::calculate(payload)).into_bytes();

        try!(self.reader.get_mut().write(&HEADER));
        try!(self.reader.get_mut().write(payload));
        try!(self.reader.get_mut().write(&crc));
        try!(self.reader.get_mut().write(&FOOTER));

        Ok(())
    }

    /// Wait until a complete message has been received
    fn receive_message_raw(&mut self) -> io::Result<Vec<u8>> {
        let mut buf = vec![];

        let _ = try!(self.reader.read_until(EOM, &mut buf));

        let header_pos = match buf.windows(HEADER.len()).position(|x| *x==HEADER) {
            None => return Err(io::Error::new(io::ErrorKind::Other,
                                              "unable to locate header in received message")),
            Some(v) => v
        };
        let footer_pos = match buf.windows(FOOTER.len()).rposition(|x| *x==FOOTER){
            None => return Err(io::Error::new(io::ErrorKind::Other,
                                              "unable to locate footer in received message")),
            Some(v) => v
        };

        // chop off header, footer and CRC
        let payload = buf.iter().take(footer_pos - CRC_SIZE).skip(header_pos + HEADER.len());
        let crc = buf.iter().skip(footer_pos - CRC_SIZE).take(CRC_SIZE);
        let crc = crc.take(4).fold(0, |acc, &item| {
            acc << 4 | match item {
                b'0' => 0, b'1' => 1, b'2' => 2,  b'3' => 3,  b'4' => 4,  b'5' => 5,  b'6' => 6,  b'7' => 7,
                b'8' => 8, b'9' => 9, b'A' => 10, b'B' => 11, b'C' => 12, b'D' => 13, b'E' => 14, b'F' => 15,
                _ => 0
            }
        });

        // CRC check
        let mut state = State::<XMODEM>::new();
        for byte in payload {
            state.update(&[*byte]);
        }

        if crc != state.get() {
            return Err(io::Error::new(io::ErrorKind::Other, "CRC error"));
        }

        let payload = buf.iter().take(footer_pos - CRC_SIZE).skip(header_pos + HEADER.len());

        Ok(payload.cloned().collect())
    }

    /// Keep receiving messages until the given message identifier has been received
    fn expect_message(&mut self, expected_message_id: u16) -> io::Result<()> {
        loop {
            let msg = try!(self.receive_message_raw());
            let decoder = PlugwiseRawData::new(&msg);

            let (decoder, msg_id) = try!(decoder.decode_u16());
            let (decoder, counter) = try!(decoder.decode_u16());
            let (decoder, mac) = if msg_id != 0 {
                try!(decoder.decode_string(16))
            } else {
                (decoder, "")
            };

            println!("type: {:04X} counter: {} mac: {}", msg_id, counter, mac);
            println!("remainder: {}", try!(decoder.remainder()));
            // TODO: message specific

            if msg_id == expected_message_id {
                break;
            }
        }

        Ok(())
    }

    /// Initialize the Plugwise USB stick
    fn initialize(&mut self) -> io::Result<()> {
        let _ = self.send_message_raw(b"000A");

        let _ = self.expect_message(0x0011);

        Ok(())
    }
}

fn run() -> io::Result<()> {
    let mut port = try!(serial::open("/dev/ttyUSB0"));
    try!(port.configure(|settings| {
        settings.set_baud_rate(serial::Baud115200);
        settings.set_char_size(serial::Bits8);
        settings.set_parity(serial::ParityNone);
        settings.set_stop_bits(serial::Stop1);
    }));

    port.set_timeout(Duration::milliseconds(1000));

    let mut plugwise = PlugwiseProtocol::new(port);

    try!(plugwise.initialize());

    Ok(())
}
