
use std::result;
use std::error;
use std::fmt;
use std::io;

pub type PlResult<T> = result::Result<T, PlError>;

/// Plugwise crate error definitions
#[derive(Debug)]
pub enum PlError {
    /// Mapped `std::io::Error`
    Io(io::Error),
    /// Plugwise USB strick reports Circle network not online
    NotOnline,
    /// Invalid timestamp from Circle
    InvalidTimestamp,
    /// Unexpected response received
    UnexpectedResponse,
    /// Protocol (i.e. CRC or formatting) error
    Protocol,
}

impl From<io::Error> for PlError {
    fn from(err: io::Error) -> PlError {
        PlError::Io(err)
    }
}

impl fmt::Display for PlError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            PlError::Io(ref err) => fmt::Display::fmt(err, f),
            PlError::NotOnline => write!(f, "Plugwise Circle network not online"),
            PlError::InvalidTimestamp => write!(f, "Circle did return a invalid timestamp"),
            PlError::UnexpectedResponse => write!(f, "Unexpected response"),
            PlError::Protocol => write!(f, "Protocol error"),
        }
    }
}

impl error::Error for PlError {
    fn description(&self) -> &str {
        match *self {
            PlError::Io(ref err) => error::Error::description(err),
            PlError::NotOnline => "Plugwise Circle network not online",
            PlError::InvalidTimestamp => "Circle did return a invalid timestamp",
            PlError::UnexpectedResponse => "Unexpected response",
            PlError::Protocol => "Protocol error",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            PlError::Io(ref err) => err.cause(),
            PlError::NotOnline => None,
            PlError::InvalidTimestamp => None,
            PlError::UnexpectedResponse => None,
            PlError::Protocol => None,
        }
    }
}
