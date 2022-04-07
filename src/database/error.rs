use core::fmt::{self, Display, Formatter};

/// Error type enum.
#[derive(Debug, PartialEq)]
pub enum ErrorKind {
	ReadOnly,
	XML,
	Serialization,
	Io,
}

/// Type representing an error emitted by the database module
pub type Error = Box<ErrorKind>;

impl Display for Error {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		let s = match &**self {
    	ErrorKind::ReadOnly => String::from("Attempted to write to read-only frequency database."),
    	ErrorKind::XML => String::from("XML Error."),
    	ErrorKind::Serialization => format!("Error during serialization."),
    	ErrorKind::Io => format!("IO Error.")
    };
    write!(f, "{}", s)
  }
}

/// Type representing a result.
pub type Result<T> = std::result::Result<T, Error>;