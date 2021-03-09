//! High level [`Connection`](connection::Connection) functionality.

pub mod id;
pub mod packet;
pub mod error;
pub mod context;
// pub mod listen;

/// Possible message that is passed by connections.
pub trait Parcel: super::byte::ByteSerialize {}

#[cfg(test)]
impl Parcel for () {}
