//! High level [`Connection`](connection::Connection) functionality.

pub mod id;
pub mod packet;
pub mod listen;
pub mod connection;

/// Possible message that is passed by connections.
pub trait Parcel: super::byte::ByteSerialize {}

#[cfg(test)]
impl Parcel for () {}
