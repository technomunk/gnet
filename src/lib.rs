//! Message-based networking over UDP for real-time applications.
// TODO: list important traits and structs

#![warn(clippy::all)]

pub mod byte;
pub mod connection;
pub mod packet;
pub mod endpoint;

use std::hash::BuildHasher;

// TODO: consider whether this is necessary
pub use connection::{Connection, PendingConnection, ConnectionError, PendingConnectionError};
pub use endpoint::{Endpoint, ClientEndpoint, ClientUdpEndpoint};

use crate::byte::ByteSerialize;

/// Possible message that is passed by connections.
pub trait Parcel : ByteSerialize {}

/// Specialized marker for stable hasher builders.
/// 
/// Stable in this case means the hashers are seeded in a constant manner on separate machines.
/// Such behavior is necessary for generating hashes (checksums) for sent data and detecting erroneous network packets.
pub trait StableBuildHasher : BuildHasher {}
