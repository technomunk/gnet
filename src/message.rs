//! Definitions of atomic messages.

use super::byte::ByteSerialize;

/// Number of allowed bytes per single serialized message payload.
pub const MAX_MESSAGE_BYTE_COUNT: usize = 256;

/// A serializeable message.
pub trait Message : ByteSerialize {}
