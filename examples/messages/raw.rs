use std::io;
use std::mem::transmute;
use super::DateTime;
use std::str;

/// Plugwise raw data consumer
pub struct RawDataConsumer<'a> {
    buf: &'a[u8],
}

impl<'a> RawDataConsumer<'a> {
    /// Wrap a buffer to consumer
    pub fn new(buf: &'a[u8]) -> RawDataConsumer<'a> {
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

    // FIXME: Rust generics doesn't allow yet to create a generic version for u8, u16, u32, u64
 
    /// Consume a `u8` from the buffer
    pub fn decode_u8(&self) -> io::Result<(RawDataConsumer, u8)> {
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
    pub fn decode_u16(&self) -> io::Result<(RawDataConsumer, u16)> {
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

    /// Consume a `u32` from the buffer
    pub fn decode_u32(&self) -> io::Result<(RawDataConsumer, u32)> {
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

    /// Consume a `f32` from the buffer
    pub fn decode_f32(&self) -> io::Result<(RawDataConsumer, f32)> {
        let (result, unconverted) = try!(self.decode_u32());
        let converted: f32 = unsafe { transmute(unconverted) };
        Ok((result, converted))
    }

    /// Consume a `u64` from the buffer
    pub fn decode_u64(&self) -> io::Result<(RawDataConsumer, u64)> {
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

    /// Consume a string of a given size from the buffer
    pub fn decode_string(&self, size: usize) -> io::Result<(RawDataConsumer, &'a str)> {
        let (buf, result) = try!(self.consume(size));

        match str::from_utf8(buf) {
            Ok(text) => Ok((result, text)),
            Err(err) => Err(io::Error::new(io::ErrorKind::Other, err))
        }
    }

    // /// Get the remainder of the data as a string
    // fn remainder(&self) -> io::Result<&'a str> {
    //     match std::str::from_utf8(self.buf) {
    //         Ok(value) => Ok(value),
    //         Err(err) => Err(io::Error::new(io::ErrorKind::Other, err))
    //     }
    // }

    pub fn decode_datetime(&self) -> io::Result<(RawDataConsumer, DateTime)> {
        let (result, _) = try!(self.decode_u32());

        let (result1, year) = try!(self.decode_u8());
        let (result1, months) = try!(result1.decode_u8());
        let (_, minutes) = try!(result1.decode_u16());

        Ok((result, DateTime::new_raw(year, months, minutes)))
    }
}

