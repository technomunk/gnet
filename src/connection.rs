//! Virtual connection with to remote access point.

mod packet;
mod connection;
mod socket;

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::hash::BuildHasher;
use std::io::{Error as IoError};

pub use connection::{Connection, PendingConnection, ConnectionError, PendingConnectionError};

use crate::byte::ByteSerialize;

/// Possible message that is passed by connections.
pub trait Parcel : ByteSerialize {}

/// Specialized marker for stable hasher builders.
/// 
/// Stable in this case means the hashers are seeded in a constant manner on separate machines.
/// Such behavior is necessary for generating hashes (checksums) for sent data and detecting erroneous network packets.
pub trait StableBuildHasher : BuildHasher {}

/// An error raised during connection process.
#[derive(Debug)]
pub enum ConnectError {
	Io(IoError),
	PayloadTooLarge,
}

impl std::convert::From<IoError> for ConnectError {
	fn from(error: IoError) -> Self {
		Self::Io(error)
	}
}

impl Display for ConnectError {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			ConnectError::Io(error) => error.fmt(f),
			ConnectError::PayloadTooLarge => write!(f, "payload too large"),
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
