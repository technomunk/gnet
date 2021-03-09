use crate::byte::SerializationError;

use std::error::Error;
use std::io::Error as IoError;

/// An error raised during connection process.
#[derive(Debug)]
pub enum ConnectError {
	Io(IoError),
	PayloadTooLarge,
}

impl From<IoError> for ConnectError {
	fn from(error: IoError) -> Self {
		Self::Io(error)
	}
}

impl std::fmt::Display for ConnectError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ConnectError::Io(error) => error.fmt(f),
			ConnectError::PayloadTooLarge => write!(f, "payload too large"),
		}
	}
}

impl PartialEq for ConnectError {
	fn eq(&self, rhs: &Self) -> bool {
		match self {
			Self::Io(lhs_error) => match rhs {
				Self::Io(rhs_error) => lhs_error.kind() == rhs_error.kind(),
				_ => false,
			},
			Self::PayloadTooLarge => matches!(rhs, Self::PayloadTooLarge),
		}
	}
}

impl Error for ConnectError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		match self {
			ConnectError::Io(error) => Some(error as &dyn Error),
			_ => None,
		}
	}
}

/// An error during the operation of a connection.
#[derive(Debug, PartialEq, Eq, PartialOrd)]
pub enum ConnectionError {
	/// The connection has no pending parcels to pop.
	NoPendingParcels,
	/// An error during deserialization of a parcel.
	Serialization(SerializationError),
	/// The connection was in an invalid state.
	InvalidState,
}

impl From<SerializationError> for ConnectionError {
	fn from(error: SerializationError) -> Self {
		Self::Serialization(error)
	}
}

impl std::fmt::Display for ConnectionError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ConnectionError::NoPendingParcels => write!(f, "no pending parcels to pop"),
			ConnectionError::InvalidState => write!(f, "the connection was in an invalid state for given operation"),
			ConnectionError::Serialization(error) => error.fmt(f),
		}
	}
}

impl Error for ConnectionError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		match self {
			ConnectionError::Serialization(error) => Some(error as &dyn Error),
			_ => None,
		}
	}
}

/// An error during invocation of [`Context::build_packet`](super::context::Context::build_packet).
#[derive(Debug, PartialEq, Eq, PartialOrd)]
pub enum BuildPacketError {
	/// The provided buffer was too small to build a packet.
	InsufficientBuffer,
	/// An error during deserialization of a parcel.
	Serialization(SerializationError),
	/// The connection was in an invalid state.
	InvalidState,
}

impl std::fmt::Display for BuildPacketError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
		match self {
			Self::InsufficientBuffer => write!(f, "the supplied buffer is too small to hold a useful packet"),
			Self::InvalidState => write!(f, "the connection is in a state that does not permit sending packets"),
			Self::Serialization(error) => {
				write!(f, "serialization error duing packet building: ");
				error.fmt(f)
			},
		}
	}
}
