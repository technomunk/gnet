//! An endpoint is one of the 2 ends of a virtual connection.
//! 
//! It may either be a simple udp socket ([`ClientUdpEndpoint`](struct.ClientUdpEndpoint.html)) or have additional demultiplexing logic ([`ServerUdpEndpoint`]()).

mod hash;
mod client;
mod server;

use std::net::SocketAddr;
use std::io::Error as IoError;

use super::connection::ConnectionId;

/// An error associated with an endpoint.
#[derive(Debug)]
pub enum TransmitError {
	/// The receiving operation would block.
	NoPendingPackets,
	/// An underlying error, different from just the non-blocking flag being set.
	Io(IoError),
}

/// A trait for objects that transmit packets across network.
/// 
/// Implementors of `Transmit` trait are called 'endpoints' or 'transmitters'.
/// 
/// 'Endpoints' are responsible for sending and receiving data packets,
/// as well as validating that received data is the sent one.
/// 
/// `Endpoints` are NOT responsible for any of the following:
/// - Packet deduplication
/// - Ordering packets
/// - Delivering packets reliably
/// 
/// **NOTE**: the `Connection` implementation assumes a UDP-like underlying protocol, specifically:
/// - All messages are sent in fixed-size packets.
/// - Packets are delivered in a best-effort manner (may be dropped).
/// - Packets are delivered in no particular order.
pub trait Transmit : Sized {
	/// Allowed payload size of the packets sent by this 'endpoint' in bytes.
	/// 
	/// **NOTE**: it should include any `` reserved by the 'endpoint'.
	const PACKET_BYTE_COUNT: usize;

	/// Number of reserved bytes by the 'endpoint'.
	/// 
	/// This many first bytes of any sent packets are left untouched by the `Connection` implementation,
	/// allowing the 'endpoint' to write checksums or other data for validating the packets.
	/// 
	/// **NOTE**: this allows avoiding an extra copy during the sending process.
	const PACKET_HEADER_BYTE_COUNT: usize;

	/// Send provided data to the provided address.
	/// 
	/// Return the number of bytes sent, which must be at least the size of `data`.
	/// Or the error responsible for the failure.
	/// 
	/// **NOTE**: only the `PACKET_HEADER_BYTE_COUNT` first bytes may be modified,
	/// the rest of the packet should stay untouched!
	fn send_to(&self, data: &mut [u8], addr: SocketAddr) -> Result<usize, IoError>;

	/// Attempt to recover all incoming packets for the connection with provided id.
	/// 
	/// Return the number of recovered bytes for the provided connection,
	/// which should be appended to provided buffer vector.
	/// Or the error responsible for the failure.
	/// Should have non-blocking behavior,
	/// meaning if there is no pending packet to recover an `Error(TransmitError::NoPendingPackets)` should be returned.
	/// 
	/// **NOTES**:
	/// - ConnectionId of `0` is a special case of `no-id`!
	/// - The 'endpoint' is expected to drop `PACKET_HEADER_BYTE_COUNT` bytes of valid incoming packets.
	fn recv_all(&self, buffer: &mut Vec<u8>, connection_id: ConnectionId) -> Result<usize, TransmitError>;
}

// Re-exports
pub use client::{ClientTransmit, ClientUdpEndpoint};
