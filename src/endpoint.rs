//! Definitions of traits an endpoint implementation must provide.
//!
//! An endpoint is one of the 2 ends of a virtual connection. It may either
//! be a simple udp socket ([`ClientEndpoint`](ClientEndpoint)) or have
//! additional demultiplexing logic ([`ServerEndpoint`](ServerEndpoint)).

mod client;
mod hash;
mod server;

use std::io::Error as IoError;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::Arc;

use super::connection::ConnectionId;

pub use client::ClientEndpoint;
#[cfg(test)]
pub(crate) use hash::TestHasherBuilder;
pub use server::ServerEndpoint;

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
pub trait Transmit {
	/// Number of bytes that sent datagrams consist of.
	///
	/// **NOTE**: it should include any
	/// [`PACKET_HEADER_BYTE_COUNT`](Self::PACKET_HEADER_BYTE_COUNT)
	/// reserved by the 'endpoint'.
	const PACKET_BYTE_COUNT: usize;

	/// Number of reserved bytes by the 'endpoint'.
	///
	/// This many first bytes of any sent packets are left untouched by the
	/// [`Connection`](super::connection::Connection) implementation, allowing
	/// the 'endpoint' to write checksums or other data for validating the packets.
	///
	/// **NOTE**: this allows avoiding an extra copy during the sending process.
	const RESERVED_BYTE_COUNT: usize;

	/// Send provided data to the provided address.
	///
	/// Return the number of bytes sent, which must be at least the size of `data`.
	/// Or the error responsible for the failure.
	///
	/// **NOTES**:
	/// - [`PACKET_HEADER_BYTE_COUNT`](Transmit::PACKET_HEADER_BYTE_COUNT) first bytes may
	/// be modified by the endpoint, the rest of the packet should stay untouched!
	/// - May expect data to be comprised of [`PACKET_BYTE_COUNT`](Self::PACKET_BYTE_COUNT) bytes.
	fn send_to(&self, data: &mut [u8], addr: SocketAddr) -> Result<usize, IoError>;

	/// Attempt to recover all incoming packets for the connection with provided id.
	///
	/// Return the number of recovered bytes for the provided connection, which should be appended
	/// to provided buffer vector. Or the error responsible for the failure. Should
	/// have non-blocking behavior, meaning if there is no pending packet to recover an
	/// [`TransmitError::NoPendingPackets`](TransmitError::NoPendingPackets) should be returned.
	///
	/// **NOTES**:
	/// - ConnectionId of `0` is a special value that means a connectionless packet.
	/// - Number of received bytes is expected to be an exact
	/// multiple of [`PACKET_BYTE_COUNT`](Self::PACKET_BYTE_COUNT).
	fn recv_all(
		&self,
		buffer: &mut Vec<u8>,
		connection_id: ConnectionId,
	) -> Result<usize, TransmitError>;
}

/// A trait for listening endpoints.
///
/// Listening endpoints require the ability to pop individual packets
/// with `0` connection id, and provide their source address.
pub trait Listen {
	/// Allow receiving packets with provided connection id.
	///
	/// By default all connection_ids except for `0` are assumed to be blocked.
	fn allow_connection_id(&self, connection_id: ConnectionId) {}

	/// Disallow receiving packets with provided connection id.
	///
	/// Undo `allow_connection_id`, allowing the endpoint to drop packets with provided
	/// connection id. By default all connection_ids except for `0` are assumed to be blocked.
	fn block_connection_id(&self, connection_id: ConnectionId) {}

	/// Remove a single packet without a connection id (`connection_id` of the packet is `0`).
	///
	/// Return the popped packet as well as its source address
	/// or a [`TransmitError`](TransmitError) The order of
	/// popping the packet is up to the implementation.
	fn pop_connectionless_packet(&self) -> Result<(SocketAddr, Box<[u8]>), TransmitError>;
}

impl std::fmt::Display for TransmitError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::NoPendingPackets => {
				write!(f, "there were no pending packets for provided connection")
			},
			Self::Io(error) => {
				write!(f, "underlying IO error: ")?;
				error.fmt(f)
			},
		}
	}
}

impl std::error::Error for TransmitError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Self::NoPendingPackets => None,
			Self::Io(error) => Some(error),
		}
	}
}

impl<T: Transmit> Transmit for Rc<T> {
	const PACKET_BYTE_COUNT: usize = T::PACKET_BYTE_COUNT;
	const RESERVED_BYTE_COUNT: usize = T::RESERVED_BYTE_COUNT;

	fn send_to(&self, data: &mut [u8], addr: SocketAddr) -> Result<usize, IoError> {
		T::send_to(self, data, addr)
	}

	fn recv_all(
		&self,
		buffer: &mut Vec<u8>,
		connection_id: ConnectionId
	) -> Result<usize, TransmitError> {
		T::recv_all(self, buffer, connection_id)
	}
}

impl<T: Transmit> Transmit for Arc<T> {
	const PACKET_BYTE_COUNT: usize = T::PACKET_BYTE_COUNT;
	const RESERVED_BYTE_COUNT: usize = T::RESERVED_BYTE_COUNT;

	fn send_to(&self, data: &mut [u8], addr: SocketAddr) -> Result<usize, IoError> {
		T::send_to(self, data, addr)
	}

	fn recv_all(
		&self,
		buffer: &mut Vec<u8>,
		connection_id: ConnectionId
	) -> Result<usize, TransmitError> {
		T::recv_all(self, buffer, connection_id)
	}
}

impl<L: Listen> Listen for Rc<L> {
	fn allow_connection_id(&self, connection_id: ConnectionId) {
		L::allow_connection_id(self, connection_id)
	}

	fn block_connection_id(&self, connection_id: ConnectionId) {
		L::block_connection_id(self, connection_id)
	}

	fn pop_connectionless_packet(&self) -> Result<(SocketAddr, Box<[u8]>), TransmitError> {
		L::pop_connectionless_packet(self)
	}
}

impl<L: Listen> Listen for Arc<L> {
	fn allow_connection_id(&self, connection_id: ConnectionId) {
		L::allow_connection_id(self, connection_id)
	}

	fn block_connection_id(&self, connection_id: ConnectionId) {
		L::block_connection_id(self, connection_id)
	}

	fn pop_connectionless_packet(&self) -> Result<(SocketAddr, Box<[u8]>), TransmitError> {
		L::pop_connectionless_packet(self)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::packet;

	use std::sync::Mutex;

	#[test]
	fn server_is_compatible_with_client() {
		let server_addr = SocketAddr::from(([127, 0, 0, 1], 1131));
		let client_addr = SocketAddr::from(([127, 0, 0, 1], 1132));

		let server = Mutex::new(ServerEndpoint::open(server_addr, TestHasherBuilder {}).unwrap());
		let client = ClientEndpoint::open(client_addr, TestHasherBuilder {}).unwrap();

		const PACKET_SIZE: usize = 1200;
		const PACKET_OFFSET: usize = 8;

		// Check test correctness
		assert_eq!(PACKET_SIZE, Mutex::<ServerEndpoint<TestHasherBuilder>>::PACKET_BYTE_COUNT);
		assert_eq!(PACKET_SIZE, ClientEndpoint::<TestHasherBuilder>::PACKET_BYTE_COUNT);
		assert_eq!(PACKET_OFFSET, Mutex::<ServerEndpoint<TestHasherBuilder>>::RESERVED_BYTE_COUNT);
		assert_eq!(PACKET_OFFSET, ClientEndpoint::<TestHasherBuilder>::RESERVED_BYTE_COUNT);

		let mut packet_header = packet::PacketHeader::request_connection(4);
		let mut packet_buffer = vec![0; PACKET_SIZE];

		packet::write_header(&mut packet_buffer[PACKET_OFFSET..], packet_header);
		packet::write_data(&mut packet_buffer[PACKET_OFFSET..], b"GNET", 0);

		let send_result = client.send_to(&mut packet_buffer, server_addr);

		assert_eq!(send_result.unwrap(), PACKET_SIZE);

		let pop_result = server.pop_connectionless_packet();

		if let Ok((addr, packet)) = pop_result {
			assert_eq!(addr, client_addr);
			assert_eq!(&packet[..], &packet_buffer[..]);
		} else {
			panic!("No packet was popped!");
		}

		packet_header.connection_id = 1;
		packet_header.packet_id = 1.into();
		server.allow_connection_id(1);

		packet::write_header(&mut packet_buffer[PACKET_OFFSET..], packet_header);
		packet::write_data(&mut packet_buffer[PACKET_OFFSET..], b"ACCEPT", 0);

		let send_result = server.send_to(&mut packet_buffer, client_addr);

		assert_eq!(send_result.unwrap(), PACKET_SIZE);

		let recv_result = client.recv_all(&mut packet_buffer, 0);

		assert_eq!(recv_result.unwrap(), PACKET_SIZE);
		assert_eq!(&packet_buffer[..PACKET_SIZE], &packet_buffer[PACKET_SIZE..]);

		packet_header.packet_id = packet_header.packet_id.next();

		packet::write_header(&mut packet_buffer[PACKET_OFFSET..], packet_header);
		packet::write_data(
			&mut packet_buffer[PACKET_OFFSET..],
			b"Testable data, shorter than 1200 bytes",
			0,
		);

		let send_result = client.send_to(&mut packet_buffer[..PACKET_SIZE], server_addr);

		assert_eq!(send_result.unwrap(), PACKET_SIZE);

		let recv_result = server.recv_all(&mut packet_buffer, 1);

		assert_eq!(recv_result.unwrap(), PACKET_SIZE);
		assert_eq!(&packet_buffer[..PACKET_SIZE], &packet_buffer[PACKET_SIZE * 2..]);
	}
}
