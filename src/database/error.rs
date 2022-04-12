use core::fmt::{self, Display, Formatter};

/// Error type enum.
#[derive(Debug, PartialEq)]
pub enum ErrorKind {
	MissingDict,
	InvalidWord,
	XML,
	Serialization,
	Io,
}

/// Type representing an error emitted by the database module
pub type Error = Box<ErrorKind>;

impl Display for Error {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		let s = match &**self {
			ErrorKind::InvalidWord => String::from("Asked for word that was not part of the dictionary."),
    	ErrorKind::MissingDict => String::from("Frequency database missing dictionary."),
    	ErrorKind::XML => String::from("XML Error."),
    	ErrorKind::Serialization => format!("Error during serialization."),
    	ErrorKind::Io => format!("IO Error.")
    };
    write!(f, "{}", s)
  }
}

/// Type representing a result.
pub type Result<T> = std::result::Result<T, Error>;