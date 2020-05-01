//! Virtual connection with to remote access point.

mod packet;
mod connection;
mod socket;

use std::io::{Error as IoError};
use std::hash::BuildHasher;

pub use connection::{Connection, PendingConnection};

use crate::byte::ByteSerialize;

/// Possible message that is passed by connections.
pub trait Parcel : ByteSerialize {}

/// Specialized marker for stable hasher builders.
/// 
/// Stable in this case means the hashers are seeded in a constant manner on separate machines.
/// Such behavior is necessary for generating hashes (checksums) for sent data and detecting erroneous network packets.
pub trait StableBuildHasher : BuildHasher {}

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
