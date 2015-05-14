use std::mem::transmute;
use super::DateTime;
use std::str;
use std::mem;
use super::super::super::error;

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
    fn consume(&self, size: usize) -> error::PlResult<(&'a[u8], RawDataConsumer)> {
        if (self.buf.len()) < size {
            return Err(error::PlError::Protocol);
        }

        let (value, remainder) = self.buf.split_at(size);

        Ok((value, RawDataConsumer {
            buf: remainder,
        }))
    }

    pub fn decode<T: Num>(&self) -> error::PlResult<(RawDataConsumer, T)> {
        let elements = mem::size_of::<T>() * 2;
        let (buf, result) = try!(self.consume(elements));

        let utf8 = unsafe {str::from_utf8_unchecked(buf)};
        let value = match Num::from_str_radix(utf8, 16) {
            Err(_) => return Err(error::PlError::Protocol),
            Ok(n) => n
        };

        Ok((result, value))
    }

    /// Consume a `f32` from the buffer
    pub fn decode_f32(&self) -> error::PlResult<(RawDataConsumer, f32)> {
        let (result, unconverted) = try!(self.decode::<u32>());
        let converted: f32 = unsafe { transmute(unconverted) };
        Ok((result, converted))
    }

    /// Consume a string of a given size from the buffer
    pub fn decode_string(&self, size: usize) -> error::PlResult<(RawDataConsumer, &'a str)> {
        let (buf, result) = try!(self.consume(size));

        match str::from_utf8(buf) {
            Ok(text) => Ok((result, text)),
            Err(_) => Err(error::PlError::Protocol)
        }
    }

    pub fn decode_datetime(&self) -> error::PlResult<(RawDataConsumer, DateTime)> {
        let (result, _) = try!(self.decode::<u32>());

        let (result1, year) = try!(self.decode::<u8>());
        let (result1, months) = try!(result1.decode::<u8>());
        let (_, minutes) = try!(result1.decode::<u16>());

        Ok((result, DateTime::new_raw(year, months, minutes)))
    }

    pub fn check_fully_consumed(&self) -> error::PlResult<()> {
        if self.buf.len() == 0 {
            Ok(())
        } else {
            Err(error::PlError::Protocol)
        }
    }

    pub fn get_remaining(&self) -> usize {
        self.buf.len()
    }
}
