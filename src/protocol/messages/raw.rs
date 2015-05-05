use std::io;
use std::mem::transmute;
use super::DateTime;
use std::str;
use std::mem;

use num::traits::Num;

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

    pub fn decode<T: Num>(&self) -> io::Result<(RawDataConsumer, T)> {
        let elements = mem::size_of::<T>() * 2;
        let (buf, result) = try!(self.consume(elements));

        let utf8 = unsafe {str::from_utf8_unchecked(buf)};
        let value = match Num::from_str_radix(utf8, 16) {
            Err(_) => return Err(io::Error::new(io::ErrorKind::Other, "raw data is not hexadecimal encoded")),
            Ok(n) => n
        };

        Ok((result, value))
    }

    /// Consume a `f32` from the buffer
    pub fn decode_f32(&self) -> io::Result<(RawDataConsumer, f32)> {
        let (result, unconverted) = try!(self.decode::<u32>());
        let converted: f32 = unsafe { transmute(unconverted) };
        Ok((result, converted))
    }

    /// Consume a string of a given size from the buffer
    pub fn decode_string(&self, size: usize) -> io::Result<(RawDataConsumer, &'a str)> {
        let (buf, result) = try!(self.consume(size));

        match str::from_utf8(buf) {
            Ok(text) => Ok((result, text)),
            Err(err) => Err(io::Error::new(io::ErrorKind::Other, err))
        }
    }

    pub fn decode_datetime(&self) -> io::Result<(RawDataConsumer, DateTime)> {
        let (result, _) = try!(self.decode::<u32>());

        let (result1, year) = try!(self.decode::<u8>());
        let (result1, months) = try!(result1.decode::<u8>());
        let (_, minutes) = try!(result1.decode::<u16>());

        Ok((result, DateTime::new_raw(year, months, minutes)))
    }

    pub fn check_fully_consumed(&self) -> io::Result<()> {
        if self.buf.len() == 0 {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "unconsumed data detected"))
        }
    }

    pub fn get_remaining(&self) -> usize {
        self.buf.len()
    }
}
