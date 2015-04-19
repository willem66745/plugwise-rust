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

/// Plugwise raw data consumer
struct RawDataConsumer<'a> {
    buf: &'a[u8],
}

impl<'a> RawDataConsumer<'a> {
    /// Wrap a buffer to consumer
    fn new(buf: &'a[u8]) -> RawDataConsumer<'a> {
        RawDataConsumer {
            buf: buf,
        }
    }

    /// Consume the buffer and create a new instance of the consumer
    fn consume(&self, size: usize) -> io::Result<(&'a[u8], RawDataConsumer)> {
        if (self.buf.len()) < size {
            return Err(io::Error::new(io::ErrorKind::Other, "data missing in received message"));
        }

        let (value, remainder) = self.buf.split_at(size);

        Ok((value, RawDataConsumer {
            buf: remainder,
        }))
    }

    // TODO: Rust generics doesn't allow yet to create a generic version for u8, u16, u32, u64
 
    /// Consume a `u8` from the buffer
    fn decode_u8(&self) -> io::Result<(RawDataConsumer, u8)> {
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

    /// Consume a `u16` from the buffer
    fn decode_u16(&self) -> io::Result<(RawDataConsumer, u16)> {
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

    // /// Consume a `u32` from the buffer
    // fn decode_u32(&self) -> io::Result<(RawDataConsumer, u32)> {
    //     let (buf, result) = try!(self.consume(8));
    //     let mut value = 0;

    //     for byte in buf {
    //         value = value << 4 | match *byte {
    //             b'0' => 0, b'1' => 1, b'2' => 2,  b'3' => 3,  b'4' => 4,  b'5' => 5,  b'6' => 6,  b'7' => 7,
    //             b'8' => 8, b'9' => 9, b'A' => 10, b'B' => 11, b'C' => 12, b'D' => 13, b'E' => 14, b'F' => 15,
    //             _ => 0
    //         };
    //     }

    //     Ok((result, value))
    // }

    /// Consume a `u64` from the buffer
    fn decode_u64(&self) -> io::Result<(RawDataConsumer, u64)> {
        let (buf, result) = try!(self.consume(16));
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

    // /// Consume a string of a given size from the buffer
    // fn decode_string(&self, size: usize) -> io::Result<(RawDataConsumer, &'a str)> {
    //     let (buf, result) = try!(self.consume(size));

    //     match std::str::from_utf8(buf) {
    //         Ok(text) => Ok((result, text)),
    //         Err(err) => Err(io::Error::new(io::ErrorKind::Other, err))
    //     }
    // }

    // /// Get the remainder of the data as a string
    // fn remainder(&self) -> io::Result<&'a str> {
    //     match std::str::from_utf8(self.buf) {
    //         Ok(value) => Ok(value),
    //         Err(err) => Err(io::Error::new(io::ErrorKind::Other, err))
    //     }
    // }
}

#[derive(Debug, Copy, Clone)]
struct ResHeader {
    msgid: MessageId,
    count: u16,
    mac: u64
}

#[derive(Debug, Copy, Clone)]
struct ResInitialize {
    unknown1: u8,
    is_online: bool,
    network_id: u64,
    short_id: u16,
    unknown2:  u8
}

impl ResInitialize {
    /// Decode initialization response
    fn new(decoder: RawDataConsumer) -> io::Result<ResInitialize> {
        let (decoder, unknown1) = try!(decoder.decode_u8());
        let (decoder, is_online) = try!(decoder.decode_u8());
        let (decoder, network_id) = try!(decoder.decode_u64());
        let (decoder, short_id) = try!(decoder.decode_u16());
        let (_, unknown2) = try!(decoder.decode_u8());

        Ok(ResInitialize {
            unknown1: unknown1,
            is_online: is_online != 0,
            network_id: network_id,
            short_id: short_id,
            unknown2: unknown2,
        })
    }
}

const EMPTY: isize = 0x0000;
const REQ_INITIALIZE: isize = 0x000A;
const RES_INITIALIZE: isize = 0x0011;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum MessageId {
    Empty = EMPTY,
    ReqInitialize = REQ_INITIALIZE,
    ResInitialize = RES_INITIALIZE
}

impl MessageId {
    fn new(id: u16) -> MessageId {
        match id as isize {
            EMPTY => MessageId::Empty,
            REQ_INITIALIZE => MessageId::ReqInitialize,
            RES_INITIALIZE => MessageId::ResInitialize,
            _ => MessageId::Empty
        }
    }

    fn as_bytes(&self) -> Vec<u8> {
        format!("{:04X}", *self as u16).bytes().collect()
    }
}

#[derive(Debug, Copy, Clone)]
enum Message {
    Empty(ResHeader),
    ReqInitialize,
    ResInitialize(ResHeader, ResInitialize)
}

impl Message {
    /// Convert given message to a bunch of bytes
    fn to_payload(&self) -> io::Result<Vec<u8>> {
        let mut vec = vec![];

        println!("> {:?}", *self); // XXX

        vec.extend(MessageId::ReqInitialize.as_bytes());

        match *self {
            Message::ReqInitialize => Ok(vec),
            _ => Err(io::Error::new(io::ErrorKind::Other, "Unsupported message type"))
        }
    }

    /// Convert given bunch of bytes to interpretable message
    fn from_payload(payload: &[u8]) -> io::Result<Message> {
        let decoder = RawDataConsumer::new(payload);

        let (decoder, msg_id) = try!(decoder.decode_u16());
        let (decoder, counter) = try!(decoder.decode_u16());
        let msg_id = MessageId::new(msg_id);

        let (decoder, mac) = if msg_id != MessageId::Empty {
            try!(decoder.decode_u64())
        } else {
            (decoder, 0)
        };

        let header = ResHeader {
            msgid: msg_id,
            count: counter,
            mac: mac
        };

        match msg_id {
            MessageId::ResInitialize => 
                Ok(Message::ResInitialize(header, try!(ResInitialize::new(decoder)))),
            _ => 
                Ok(Message::Empty(header))
        }
    }

    fn to_message_id(&self) -> MessageId {
        match *self {
            Message::Empty(..) => MessageId::Empty,
            Message::ReqInitialize(..) => MessageId::ReqInitialize,
            Message::ResInitialize(..) => MessageId::ResInitialize,
        }
    }
}

struct Protocol<R> {
    reader: io::BufReader<R>
}

impl<R: Read + Write> Protocol<R> {
    /// Wrap IO entity for Plugwise protocol handling
    fn new(port: R) -> Protocol<R> {
        Protocol {
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
    fn expect_message(&mut self, expected_message_id: MessageId) -> io::Result<Message> {
        loop {
            let msg = try!(self.receive_message_raw());
            let msg = try!(Message::from_payload(&msg));

            println!("< {:?}", msg); // XXX

            if msg.to_message_id() == expected_message_id {
                return Ok(msg)
            }
        }
    }

    /// Initialize the Plugwise USB stick
    fn initialize(&mut self) -> io::Result<bool> {
        let msg = try!(Message::ReqInitialize.to_payload());
        try!(self.send_message_raw(&msg));

        let msg = try!(self.expect_message(MessageId::ResInitialize));

        match msg {
            Message::ResInitialize(_, res) => Ok(res.is_online),
            _ => Err(io::Error::new(io::ErrorKind::Other, "unexpected initialization response"))
        }
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

    let mut plugwise = Protocol::new(port);

    let _ = try!(plugwise.initialize());

    Ok(())
}

fn main() {
    run().unwrap();
}
