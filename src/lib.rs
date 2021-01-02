//! Message-based networking over UDP for real-time applications.
// TODO: list important traits and structs

#![warn(clippy::all)]

#![cfg_attr(debug_assertions, allow(dead_code, unused_imports, unused_variables))]

pub mod byte;
pub mod connection;
pub mod packet;
pub mod endpoint;
pub mod listener;

use std::hash::BuildHasher;

// TODO: consider whether this is necessary
pub use connection::{Connection, PendingConnection, ConnectionError, PendingConnectionError};
pub use endpoint::{Transmit, Listen, ClientUdpEndpoint, ServerUdpEndpoint};

use crate::byte::ByteSerialize;

/// Possible message that is passed by connections.
pub trait Parcel : ByteSerialize {}

/// Specialized marker for stable hasher builders.
/// 
/// Stable in this case means the hashers are seeded in a constant manner on separate machines.
/// Such behavior is necessary for generating hashes (checksums) for sent data and detecting erroneous network packets.
pub trait StableBuildHasher : BuildHasher {}

/// Hasher used in unit tests throughout the library.
#[cfg(test)]
pub(crate) type TestHasher = hashers::fnv::FNV1aHasher32;

#[cfg(test)]
pub(crate) struct TestHasherBuilder();

#[cfg(test)]
impl BuildHasher for TestHasherBuilder {
	type Hasher = TestHasher;
	fn build_hasher(&self) -> Self::Hasher { TestHasher::default() }
}

#[cfg(test)]
impl StableBuildHasher for TestHasherBuilder {}
