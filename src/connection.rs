//! High level [`Connection`](connection::Connection) functionality.

pub mod id;
pub mod parcel;
pub mod error;
pub mod context;
pub mod listen;
pub mod track;

/// Possible message that is passed by connections.
pub trait Parcel: super::byte::ByteSerialize {}

#[cfg(test)]
impl Parcel for () {}
