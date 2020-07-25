//! An endpoint is one of the 2 ends of a virtual connection.
//! 
//! It may either be a simple udp socket ([`ClientUdpEndpoint`](struct.ClientUdpEndpoint.html)) or have additional demultiplexing logic ([`ServerUdpEndpoint`]()).

mod client;
mod server;

use std::net::{SocketAddr, UdpSocket};
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::iter::repeat;

use super::connection::ConnectionId;
use super::StableBuildHasher;

/// An error associated with an endpoint.
#[derive(Debug)]
pub enum EndpointError {
	/// Ie WouldBlock.
	NoPendingPackets,
	/// An underlying error.
	Io(IoError),
}

/// A trait for a connection endpoint.
/// 
/// The `Endpoint` is responsible for sending and receiving messages for [`Connections`]().
/// **NOTE**: the Connection implementation assumes a UDP-like underlying protocol, specifically:
/// - All messages are send in fixed-size packets.
/// - Packets are delivered in a best-effort manner (may be dropped).
/// - Packets are delivered in no particular order.
pub trait Endpoint : Sized {
	/// Send provided data to the provided address.
	/// 
	/// Return the number of bytes sent, which must be at least the size of `data`.
	/// Or the error responsible for the failure.
	fn send_to(&self, data: &[u8], addr: SocketAddr) -> Result<usize, IoError>;

	/// Attempt to recover all incoming packets for the connection with provided id.
	/// 
	/// Return the number of recovered bytes for the provided connection,
	/// which should be appended to provided buffer vector.
	/// Or the error responsible for the failure.
	/// Should have non-blocking behavior,
	/// meaning if there is no pending packet to recover an `Error(EnpointError::NoPendingPackets)` should be returned.
	/// 
	/// **NOTE**: ConnectionId of `0` is a special case of `no-id`!
	fn recv_all<H: StableBuildHasher>(&self, buffer: &mut Vec<u8>, connection_id: ConnectionId, hash_builder: &H) -> Result<usize, EndpointError>;

	/// Open a new Endpoint with provided local address.
	fn open(addr: SocketAddr) -> Result<Self, IoError>;
}

// Re-exports
pub use client::{ClientEndpoint, ClientUdpEndpoint};
