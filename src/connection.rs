//! Definitions of connection-related structs. This is the primary export of the library.

#![cfg_attr(debug_assertions, allow(dead_code, unused_imports, unused_variables))]

mod error;

pub use error::{ConnectError, ConnectionError, PendingConnectionError};

use crate::byte::{ByteSerialize, SerializationError};
use crate::id::ConnectionId;

use super::endpoint::{Demux, Transmit, TransmitError};
use super::packet;
use super::packet::PacketHeader;
use super::Parcel;

use rand::random;

use std::mem::size_of;
use std::iter::repeat;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

const RESYNC_PERIOD: Duration = Duration::from_millis(200);

/// State of a [Connection](Connection).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ConnectionStatus {
	/// Normal functioning state.
	///
	/// `Connection`'s full functionality may be used.
	Open,
	/// `Connection` has beed deemed lost, due to lack of received relevant network traffic.
	/// This may be caused by a sudden shutdown of the other end or due to network conditions.
	///
	/// `Connection` may be demoted to a `PendingConnection` or dropped.
	Lost,
	/// `Connection` has been explicitly closed by the other end.
	///
	/// `Connection` may only be dropped to free system resources.
	Closed,
}

/// A virtual connection with to remote access point.
///
/// This connection is not backed by a stable route (like TCP connections), however it
/// still provides similar functionality.
///
/// # Generic Parameters
///
/// - P: [Parcel](super::Parcel) type of passed messages used by this [`Connection`](Self).
/// - H: [StableBuildHasher](super::StableBuildHasher) hasher provided for the connection that is
/// used to validate transmitted packets.
///
/// *NOTE: messages with incorrect hash are immediately discarded, meaning both ends of a
/// connection need to have exact same `BuildHasher`. It is recommended to seed the hasher
/// with a unique secret seed for the application.
#[derive(Debug)]
pub struct Connection<T: Transmit, P: Parcel> {
	endpoint: T,
	connection_id: ConnectionId,
	remote: SocketAddr,
	packet_buffer: Vec<u8>,
	status: ConnectionStatus,
	last_sent_packet_time: Instant,
	last_received_packet_time: Instant,

	// TODO/https://github.com/rust-lang/rust/issues/43408 : use [u8; T::PACKET_BYTE_COUNT] instead of Vec<u8>.
	sent_packet_buffer: Vec<(Instant, Vec<u8>)>,
	// TODO: connection-accept should be a synchronized packet with id 0.
	received_packet_ack_id: packet::PacketIndex,
	received_packet_ack_mask: u64,

	_message_type: PhantomData<P>,
}

impl<T: Transmit, P: Parcel> Connection<T, P> {
	/// Construct a connection in an open state.
	pub(crate) fn opened(endpoint: T, connection_id: ConnectionId, remote: SocketAddr) -> Self {
		let now = Instant::now();
		Self {
			endpoint,
			connection_id,
			remote,
			packet_buffer: Vec::with_capacity(T::MAX_FRAME_LENGTH),
			status: ConnectionStatus::Open,
			last_sent_packet_time: now,
			last_received_packet_time: now,

			sent_packet_buffer: Vec::with_capacity(65),
			received_packet_ack_id: Default::default(),
			received_packet_ack_mask: 0,

			_message_type: PhantomData,
		}
	}
}

impl<E: Transmit, P: Parcel> Connection<E, P> {
	const MAX_PAYLOAD_LENGTH: usize = E::MAX_FRAME_LENGTH - size_of::<packet::PacketHeader>();

	/// Attempt to establish a new connection to provided remote address from provided local one.
	#[inline]
	pub fn connect(
		endpoint: E,
		remote: SocketAddr,
		payload: Vec<u8>,
	) -> Result<PendingConnection<E, P>, ConnectError> {
		if payload.len() > Self::MAX_PAYLOAD_LENGTH {
			Err(ConnectError::PayloadTooLarge)
		} else {
			let handshake_id = random::<u32>().to_ne_bytes();
			let mut packet_buffer = Vec::with_capacity(E::MAX_FRAME_LENGTH);
			packet_buffer.resize_with(payload.len() + size_of::<packet::PacketHeader>(), Default::default);
			packet::write_header(
				&mut packet_buffer,
				PacketHeader::request_connection(handshake_id, payload.len() as u16),
			);
			if !payload.is_empty() {
				packet::write_data(&mut packet_buffer, &payload, 0);
			}
			endpoint.send_to(&packet_buffer, remote)?;
			packet_buffer.clear();
			let communication_time = Instant::now();
			Ok(PendingConnection {
				endpoint,
				remote,
				packet_buffer,
				last_sent_packet_time: communication_time,
				last_communication_time: communication_time,
				payload,
				handshake_id,
				_message_type: PhantomData,
			})
		}
	}

	/// Get the current status (state) of the `Connection`.
	#[inline]
	pub fn status(&self) -> ConnectionStatus {
		self.status
	}

	/// Checks that the [`Connection`](Self) is in [`Open`](ConnectionStatus::Open) (normal) state.
	///
	/// *Note: this only queries the current status of the connection, the
	/// connection may still fail after [`is_open()`](Self::is_open) returned true.*
	#[inline]
	pub fn is_open(&self) -> bool {
		self.status == ConnectionStatus::Open
	}

	/// Get the next parcel from the connection.
	///
	/// Includes the data prelude from the network packet the parcel was transmitted with. Will query
	/// the socket, pop any pending network packets and finally pop a parcel.
	///
	/// # Note
	/// Prefer using [`pop_mux_parcel`](Connection::pop_mux_parcel) if possible, as it allows multiple
	/// connections to share the same endpoint.
	pub fn pop_parcel(&mut self) -> Result<(P, [u8; 4]), ConnectionError> {
		todo!()
	}

	/// Get the next parcel from the connection.
	///
	/// Includes the data prelude from the network packet the parcel was transmitted with. Will query
	/// the socket, pop any pending network packets and finally pop a parcel.
	///
	/// # Note
	/// Behaves similarly to [`pop_parcel`](Connection::pop_parcel), except demultiplexes read packets
	/// allowing multiple connection to share the same endpoint.
	pub fn pop_mux_parcel(&mut self) -> Result<(P, [u8; 4]), ConnectError> where
		E: Demux<ConnectionId>,
	{
		todo!()
	}

	/// Begin reliable transmission of provided parcel.
	///
	/// Reliable parcels are guaranteed to be delivered as long as the connection
	/// is in a valid state. The order of delivery is not guaranteed however, for
	/// order-dependent functionality use streams.
	///
	/// # Notes
	/// - May result in network packet dispatch.
	pub fn push_reliable_parcel(&mut self, parcel: P) -> Result<(), ConnectionError> {
		todo!()
	}

	/// Begin unreliable transmission of provided parcel.
	///
	/// Unreliable (volatile) parcels are delivered in a best-effort manner, however no
	/// re-transmission occurs of the parcel was not received by the other end. The order
	/// of delivery is not guaranteed, for order-dependent functionality use streams.
	///
	/// # Notes
	/// - May result in network packet dispatch.
	pub fn push_volatile_parcel(&mut self, parcel: P) -> Result<(), ConnectionError> {
		todo!()
	}

	/// Write a given slice of bytes to the connection stream.
	///
	/// # Streams
	/// Connection streams offer
	/// [TCP](https://en.wikipedia.org/wiki/Transmission_Control_Protocol)-like functionality
	/// for contiguous streams of data. Streams are transmitted with the same network packets
	/// as reliable parcels, reducing overall data duplication for lost packets.
	///
	/// # Notes
	/// - May result in network packet dispatch.
	pub fn write_bytes_to_stream(&mut self, bytes: &[u8]) -> Result<(), ConnectionError> {
		todo!()
	}

	/// Write a given byte-serializeable item to the connection stream.
	///
	/// # Returns
	/// Number of bytes written.
	///
	/// # Streams
	/// Connection streams offer
	/// [TCP](https://en.wikipedia.org/wiki/Transmission_Control_Protocol)-like functionality
	/// for contiguous streams of data. Streams are transmitted with the same network packets
	/// as reliable parcels, reducing overall data duplication for lost packets.
	///
	/// # Notes
	/// - May result in network packet dispatch.
	pub fn write_item_to_stream<B: ByteSerialize>(
		&mut self,
		item: &B,
	) -> Result<usize, ConnectionError> {
		todo!()
	}

	/// Attempt to read data from the connection stream into the provided buffer.
	///
	/// # Returns
	/// Number of bytes read.
	///
	/// # Streams
	/// Connection streams offer
	/// [TCP](https://en.wikipedia.org/wiki/Transmission_Control_Protocol)-like functionality
	/// for contiguous streams of data. Streams are transmitted with the same network packets
	/// as reliable parcels, reducing overall data duplication for lost packets.
	///
	/// # Notes
	/// - Will not read past the end of the provided buffer.
	/// - Prefer using [`read_from_mux_stream`](Connection::read_from_mux_stream), which demultiplexes
	/// incoming packets, allowing multiple connections to share the same endpoint.
	pub fn read_from_stream(&mut self, buffer: &mut [u8]) -> Result<usize, ConnectionError> {
		todo!()
	}

	/// Attempt to read data from the connection stream into the provided buffer.
	///
	/// # Returns
	/// Number of bytes read.
	///
	/// # Streams
	/// Connection streams offer
	/// [TCP](https://en.wikipedia.org/wiki/Transmission_Control_Protocol)-like functionality
	/// for contiguous streams of data. Streams are transmitted with the same network packets
	/// as reliable parcels, reducing overall data duplication for lost packets.
	///
	/// # Notes
	/// - Will not read past the end of the provided buffer.
	/// - Behaves similarly to [`read_from_stream`](Connection::read_from_stream) except this also
	/// demultiplexes incoming packets, allowing multiple connections to share the same endpoint.
	pub fn read_from_mux_stream(&mut self, buffer: &mut [u8]) -> Result<usize, ConnectionError> where
		E: Demux<ConnectionId>,
	{
		todo!()
	}

	/// Query the amount of bytes ready to be read from the incoming stream.
	///
	/// # Streams
	/// Connection streams offer
	/// [TCP](https://en.wikipedia.org/wiki/Transmission_Control_Protocol)-like functionality
	/// for contiguous streams of data. Streams are transmitted with the same network packets
	/// as reliable parcels, reducing overall data duplication for lost packets.
	///
	/// # Notes
	/// - Does not do synchronization that [`read_from_stream()`](Self::read_from_stream) performs,
	/// as a result there may be more bytes ready to be read than returned.
	pub fn pending_incoming_stream_bytes(&self) -> Result<usize, ConnectionError> {
		todo!()
	}

	/// Flush any outgoing packets.
	/// 
	/// # Notes
	/// Flushing may cause loss of efficiency in network utilization, as the sent packets may
	/// not be fully filled.
	pub fn flush(&mut self) -> Result<(), ConnectionError> {
		todo!()
	}
}

impl<T: Transmit, P: Parcel> PartialEq for Connection<T, P> {
	fn eq(&self, rhs: &Self) -> bool {
		self.connection_id == rhs.connection_id && self.remote == rhs.remote
	}
}

/// A temporary connection that is in the process of being established for the first time.
///
/// Primary purpose is to be promoted to a full connection once established or dropped on timeout.
#[derive(Debug)]
pub struct PendingConnection<T: Transmit, P: Parcel> {
	endpoint: T,
	remote: SocketAddr,
	packet_buffer: Vec<u8>,
	last_sent_packet_time: Instant,
	last_communication_time: Instant,
	payload: Vec<u8>,
	handshake_id: packet::DataPrelude,

	_message_type: PhantomData<P>,
}

impl<T: Transmit, P: Parcel> PendingConnection<T, P> {
	/// Attempt to promote the pending connection to a full [`Connection`](Connection).
	///
	/// Receives any pending network packets, promoting the connection to a full
	/// [`Connection`](Connection) if valid GNet packets were received.
	pub fn try_promote(self) -> Result<Connection<T, P>, PendingConnectionError<T, P>> {
		todo!()
	}

	/// Get the span of time passed since the last request for the connection has been sent.
	#[inline]
	pub fn time_since_last_request(&self) -> Duration {
		Instant::now().duration_since(self.last_sent_packet_time)
	}

	/// Update the pending connection.
	///
	/// - Reads any pending network packets, filtering them.
	/// - If no packets have been received for half a timeout window re-sends the request.
	pub fn sync(&mut self) -> Result<(), PendingConnectionError<T, P>> {
		todo!()
	}
}
