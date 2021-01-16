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
pub use endpoint::{Transmit, Listen, ClientEndpoint, ServerEndpoint};

use crate::byte::ByteSerialize;

/// Possible message that is passed by connections.
pub trait Parcel : ByteSerialize {}
