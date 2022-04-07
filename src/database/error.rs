use core::fmt::{self, Display, Formatter};

/// Error type enum.
#[derive(Debug)]
pub enum ErrorKind {
	ReadOnly,
	XML,
	Serialization(bincode::Error),
	Io(std::io::Error),
}

/// Type representing an error emitted by the database module
pub type Error = Box<ErrorKind>;

impl Display for Error {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		let s = match &**self {
    	ErrorKind::ReadOnly => String::from("Attempted to write to read-only frequency database."),
    	ErrorKind::XML => String::from("XML Error."),
    	ErrorKind::Serialization(x) => format!("Error during serialization: {}", x),
    	ErrorKind::Io(x) => format!("IO Error: {}", x)
    };
    write!(f, "{}", s)
  }
}

/// Type representing a result.
pub type Result<T> = std::result::Result<T, Error>;