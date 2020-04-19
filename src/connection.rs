//! Virtual connection with to remote access point.

mod packet;
mod connection;

use std::io::{Error as IoError};

pub use connection::{Connection, PendingConnection};

use crate::byte::ByteSerialize;

/// Possible message that is passed by connections.
pub trait Parcel : ByteSerialize {}

/// An error raised during connection process.
pub enum ConnectError {
	Io(IoError),
	PayloadTooLarge,
}

impl std::convert::From<IoError> for ConnectError {
	fn from(error: IoError) -> Self {
		Self::Io(error)
	}
}
