//! [`Demux`](Demux) trait definition, implementation and test.

#[cfg(feature = "basic-endpoints")]
pub mod basic;
#[cfg(test)]
pub mod test;

pub use crate::id::ConnectionId;

use super::Transmit;

use std::net::SocketAddr;
use std::sync::Mutex;

/// A trait for connection demultiplexers.
///
/// Demultiplexers allow multiple connections to use the same endpoint simultaneously by
/// inspecting arriving datagrams and buffering any that belong to allowed connections for
/// later use.
pub trait Demux {
	/// Allow buffering datagrams associated with provided connection.
	fn allow(&mut self, connection: ConnectionId);
	/// Block (disallow) buffering datagrams associated with provided connection.
	///
	/// # Note
	/// Any non-popped datagrams associated with newly blocked connection should be dropped.
	fn block(&mut self, connection: ConnectionId);
	/// Check whether buffering datagrams associated with provided connection is currently allowed.
	fn is_allowed(&self, connection: ConnectionId) -> bool;

	/// Buffer a datagram associated with provided connection.
	///
	/// # Notes
	/// - The length and source address of the datagram should be recorded as it needs
	/// to be returned with [`pop`](Mux::pop).
	/// - The connection may be assumed to be allowed at the time of invocation.
	fn push(&mut self, connection: ConnectionId, addr: SocketAddr, datagram: &[u8]);
	/// Remove a buffered datagram associated with provided connection if any were buffered.
	///
	/// The datagram should be written to provided buffer and the function should return the length
	/// of the popped datagram or `None`.
	/// 
	/// # Notes
	/// - The datagram order does not need to be preserved, meaning popped datagrams may come
	/// in a different order to push().
	/// - The connection may be assumed to be allowed at the time of invocation.
	fn pop(&mut self, connection: ConnectionId, buffer: &mut [u8]) -> Option<(usize, SocketAddr)>;
}
