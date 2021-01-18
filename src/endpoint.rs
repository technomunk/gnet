//! Definitions of traits an endpoint implementation must provide.
//!
//! An endpoint is one of the 2 ends of a virtual connection. It may either
//! be a simple udp socket ([`ClientEndpoint`](ClientEndpoint)) or have
//! additional demultiplexing logic ([`ServerEndpoint`](ServerEndpoint)).

#[cfg(feature = "basic-endpoints")]
pub mod basic;

use std::io::Error as IoError;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::Arc;

use super::id::ConnectionId;

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

/// A trait for endpoints that may be initialized with address only.
pub trait Open: Sized {
	/// Attempt to bind a new endpoint at a provided local address.
	fn open(addr: SocketAddr) -> Result<Self, IoError>;
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
pub mod test {
	use super::*;
	use crate::packet;

	#[inline]
	fn get_packet_id<T: Transmit>(packet: &[u8]) -> packet::PacketIndex {
		packet::get_header(&packet[T::RESERVED_BYTE_COUNT ..]).packet_id
	}

	/// Test that 2 endpoints are able to send GNet packets to each other.
	pub fn generic_send_and_receive_test<A: Transmit, B: Transmit>(
		(a, a_addr): (A, SocketAddr),
		(b, b_addr): (B, SocketAddr),
	) {
		assert_eq!(A::PACKET_BYTE_COUNT, B::PACKET_BYTE_COUNT);
		assert_eq!(A::RESERVED_BYTE_COUNT, B::RESERVED_BYTE_COUNT);
		const PAYLOAD_DATA: &[u8] = b"TEST DATA";

		let mut packet_header = packet::PacketHeader {
			connection_id: 1,
			packet_id: 1.into(),
			ack_packet_id: Default::default(),
			ack_packet_mask: 0,
			signal: packet::SignalBits::volatile(PAYLOAD_DATA.len() as u16),
			prelude: [ 1, 2, 3, 4, ],
		};

		let mut packet_buffer = vec![0; A::PACKET_BYTE_COUNT];

		// Send just 1 packet A to B

		packet::write_header(&mut packet_buffer[A::RESERVED_BYTE_COUNT ..], packet_header);
		packet::write_data(&mut packet_buffer[A::RESERVED_BYTE_COUNT ..], PAYLOAD_DATA, 0);

		assert_eq!(A::PACKET_BYTE_COUNT, a.send_to(&mut packet_buffer, b_addr).unwrap());

		packet_buffer.clear();
		let recv_result = b.recv_all(&mut packet_buffer, 1);

		let packet = &packet_buffer[A::RESERVED_BYTE_COUNT ..];
		assert_eq!(recv_result.unwrap(), A::PACKET_BYTE_COUNT);
		assert_eq!(packet_buffer.len(), A::PACKET_BYTE_COUNT);
		assert_eq!(*packet::get_header(packet), packet_header);
		assert_eq!(packet::get_parcel_segment(packet), PAYLOAD_DATA);

		// Send 2 packets B to A

		packet_header.packet_id = packet_header.packet_id.next();
		packet::write_header(&mut packet_buffer[B::RESERVED_BYTE_COUNT ..], packet_header);

		assert_eq!(b.send_to(&mut packet_buffer, a_addr).unwrap(), A::PACKET_BYTE_COUNT);

		packet_header.packet_id = packet_header.packet_id.next();
		packet::write_header(&mut packet_buffer[B::RESERVED_BYTE_COUNT ..], packet_header);

		assert_eq!(b.send_to(&mut packet_buffer, a_addr).unwrap(), A::PACKET_BYTE_COUNT);

		assert_eq!(a.recv_all(&mut packet_buffer, 1).unwrap(), A::PACKET_BYTE_COUNT * 2);
		assert_eq!(packet_buffer.len(), A::PACKET_BYTE_COUNT * 3);

		let packet = &packet_buffer[A::PACKET_BYTE_COUNT .. A::PACKET_BYTE_COUNT * 2];
		assert_eq!(get_packet_id::<A>(packet), 2.into());
		assert_eq!(packet::get_parcel_segment(packet), PAYLOAD_DATA);

		let packet = &packet_buffer[A::PACKET_BYTE_COUNT * 2 ..];
		assert_eq!(get_packet_id::<A>(packet), 3.into());
		assert_eq!(packet::get_parcel_segment(packet), PAYLOAD_DATA);
	}

	/// Test that an endpoint is able to accept another endpoint's GNet requests.
	pub fn generic_accept_test<S: Transmit + Listen, C: Transmit>(
		(server, server_addr): (S, SocketAddr),
		(client, client_addr): (C, SocketAddr),
	) {
		assert_eq!(S::PACKET_BYTE_COUNT, C::PACKET_BYTE_COUNT);
		assert_eq!(S::RESERVED_BYTE_COUNT, C::RESERVED_BYTE_COUNT);
		const REQUEST_DATA: &[u8] = b"GNET REQUEST";
		const ACCEPT_DATA: &[u8] = b"GNET ACCEPT";
		const PAYLOAD_DATA: &[u8] = b"GNET PAYLOAD DATA";

		let mut packet_header = packet::PacketHeader::request_connection(4);
		let mut packet_buffer = vec![0; S::PACKET_BYTE_COUNT];
		
		packet_header.signal.set_parcel_byte_count(REQUEST_DATA.len() as u16);
		packet::write_header(&mut packet_buffer[S::RESERVED_BYTE_COUNT ..], packet_header);
		packet::write_data(&mut packet_buffer[S::RESERVED_BYTE_COUNT ..], REQUEST_DATA, 0);

		assert_eq!(client.send_to(&mut packet_buffer, server_addr).unwrap(), S::PACKET_BYTE_COUNT);

		let pop_result = server.pop_connectionless_packet();

		if let Ok((addr, packet)) = pop_result {
			assert_eq!(addr, client_addr);
			assert_eq!(
				packet::get_parcel_segment(&packet[S::RESERVED_BYTE_COUNT ..]),
				REQUEST_DATA,
			)
		} else {
			panic!("No packet was popped!");
		}

		packet_header.connection_id = 1;
		packet_header.packet_id = 1.into();
		server.allow_connection_id(1);

		packet_header.signal.set_parcel_byte_count(ACCEPT_DATA.len() as u16);
		packet::write_header(&mut packet_buffer[S::RESERVED_BYTE_COUNT ..], packet_header);
		packet::write_data(&mut packet_buffer[S::RESERVED_BYTE_COUNT ..], ACCEPT_DATA, 0);

		assert_eq!(server.send_to(&mut packet_buffer, client_addr).unwrap(), S::PACKET_BYTE_COUNT);

		assert_eq!(client.recv_all(&mut packet_buffer, 0).unwrap(), S::PACKET_BYTE_COUNT);
		assert_eq!(
			packet::get_parcel_segment(&packet_buffer[S::PACKET_BYTE_COUNT + S::RESERVED_BYTE_COUNT ..]),
			ACCEPT_DATA,
		);


		packet_header.packet_id = packet_header.packet_id.next();
		packet_header.signal.set_parcel_byte_count(PAYLOAD_DATA.len() as u16);
		packet::write_header(&mut packet_buffer[S::RESERVED_BYTE_COUNT ..], packet_header);
		packet::write_data(&mut packet_buffer[S::RESERVED_BYTE_COUNT ..], PAYLOAD_DATA, 0);

		let send_result = client.send_to(&mut packet_buffer[.. S::PACKET_BYTE_COUNT], server_addr);
		assert_eq!(send_result.unwrap(), S::PACKET_BYTE_COUNT);

		assert_eq!(server.recv_all(&mut packet_buffer, 1).unwrap(), S::PACKET_BYTE_COUNT);
		assert_eq!(
			packet::get_parcel_segment(&packet_buffer[S::PACKET_BYTE_COUNT * 2 ..]),
			PAYLOAD_DATA,
		);
	}
}
