//! High level [`Connection`](connection::Connection) functionality.

pub mod ack;
pub mod context;
pub mod deliver;
pub mod error;
pub mod id;
pub mod listen;
pub mod parcel;

/// Possible message that is passed by connections.
pub trait Parcel: super::byte::ByteSerialize {}

#[cfg(test)]
impl Parcel for () {}
