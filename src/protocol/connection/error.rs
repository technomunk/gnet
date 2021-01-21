use crate::protocol::Parcel;
use crate::byte::SerializationError;
use crate::endpoint::Transmit;

use super::PendingConnection;

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

/// An error during the operation of a [`Connection`](Connection).
#[derive(Debug)]
pub enum ConnectionError {
	/// The connection has no pending parcels to pop.
	NoPendingParcels,
	/// An error during deserialization of a parcel.
	Serialization(SerializationError),
	/// The connection was in an invalid state.
	InvalidState,
	/// An unexpected IO error ocurred.
	Io(IoError),
}

impl From<IoError> for ConnectionError {
	fn from(error: IoError) -> ConnectionError {
		Self::Io(error)
	}
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
			ConnectionError::Io(error) => error.fmt(f),
		}
	}
}

impl PartialEq for ConnectionError {
	fn eq(&self, rhs: &Self) -> bool {
		match self {
			Self::Io(lhs_error) => match rhs {
				Self::Io(rhs_error) => lhs_error.kind() == rhs_error.kind(),
				_ => false,
			},
			other => matches!(other, rhs)
		}
	}
}

impl Error for ConnectionError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		match self {
			ConnectionError::Serialization(error) => Some(error as &dyn Error),
			ConnectionError::Io(error) => Some(error as &dyn Error),
			_ => None,
		}
	}
}

/// An error specific to a pending connection.
#[derive(Debug)]
pub enum PendingConnectionError<T: Transmit, P: Parcel> {
	/// No answer has yet been received.
	NoAnswer(PendingConnection<T, P>),
	/// The answer has been received, but it was incorrect.
	InvalidAnswer(PendingConnection<T, P>),
	/// An unexpected IO error ocurred.
	Io(IoError),
	/// The connection has been actively rejected by the other end (and subsequently consumed).
	Rejected,
}

impl<T: Transmit, P: Parcel> From<IoError> for PendingConnectionError<T, P> {
	fn from(error: IoError) -> Self {
		PendingConnectionError::Io(error)
	}
}

impl<T: Transmit, P: Parcel> std::fmt::Display for PendingConnectionError<T, P> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			PendingConnectionError::NoAnswer(_) => write!(f, "no answer has yet been received"),
			PendingConnectionError::InvalidAnswer(_) => write!(f, "an misformed answer has been received"),
			PendingConnectionError::Io(error) => error.fmt(f),
			PendingConnectionError::Rejected => write!(f, "connection has been actively rejected by the other end"),
		}
	}
}

impl<T: Transmit, P: Parcel> PartialEq for PendingConnectionError<T, P> {
	fn eq(&self, rhs: &Self) -> bool {
		match self {
			Self::Io(lhs_error) => match rhs {
				Self::Io(rhs_error) => lhs_error.kind() == rhs_error.kind(),
				_ => false,
			},
			other => matches!(other, rhs)
		}
	}
}

impl<T, P> Error for PendingConnectionError<T, P>
where
	T: Transmit + std::fmt::Debug,
	P: Parcel + std::fmt::Debug,
{
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		match self {
			PendingConnectionError::Io(error) => Some(error as &dyn Error),
			_ => None,
		}
	}
}
