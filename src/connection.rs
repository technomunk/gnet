//! Definitions of connection-related structs. This is the primary export of the library.

use crate::byte::{ByteSerialize, SerializationError};

use super::Parcel;
use super::packet;
use super::packet::PacketHeader;
use super::endpoint::{Transmit, TransmitError};

use std::error::Error;
use std::io::{Error as IoError};
use std::marker::PhantomData;
use std::time::{Duration, Instant};
use std::net::SocketAddr;
use std::iter::repeat;

/// A unique index associated with a connection.
/// 
/// **NOTE**: `0` is a special value that means `no-connection-id`.
pub(super) type ConnectionId = u16;

const RESYNC_PERIOD: Duration = Duration::from_millis(200);

/// An error raised during connection process.
#[derive(Debug)]
pub enum ConnectError {
	Io(IoError),
	PayloadTooLarge,
}

/// An error during the operation of a [`Connection`](Connection).
#[derive(Debug)]
pub enum ConnectionError {
	/// The connection has no pending parcels to pop.
	NoPendingParcels,
	/// An error during deserialization of a parcel.
	Serialization(SerializationError),
	/// The connection was in an invalid state.
	InvalidState,
	/// An unexpected IO error ocurred.
	Io(IoError),
}

/// An error specific to a pending connection.
#[derive(Debug)]
pub enum PendingConnectionError<T: Transmit, P: Parcel> {
	/// No answer has yet been received.
	NoAnswer(PendingConnection<T, P>),
	/// The answer has been received, but it was incorrect.
	InvalidAnswer(PendingConnection<T, P>),
	/// An unexpected IO error ocurred.
	Io(IoError),
	/// The connection has been actively rejected by the other end (and subsequently consumed).
	Rejected,
	/// The predicate passed to `try_promote()` returned false.
	PredicateFail,
}

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
/// This connection is not backed by a stable route (like TCP connections),
/// however it still provides similar functionality.
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
			packet_buffer: Vec::with_capacity(T::PACKET_BYTE_COUNT),
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

	_message_type: PhantomData<P>,
}

impl std::convert::From<IoError> for ConnectError {
	fn from(error: IoError) -> Self {
		Self::Io(error)
	}
}

impl std::fmt::Display for ConnectError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ConnectError::Io(error) => error.fmt(f),
			ConnectError::PayloadTooLarge => write!(f, "payload too large"),
		}
	}
}

impl Error for ConnectError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		match self {
			ConnectError::Io(error) => Some(error as &dyn Error),
			_ => None,
		}
	}
}

impl std::fmt::Display for ConnectionError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ConnectionError::NoPendingParcels => write!(f, "no pending parcels to pop"),
			ConnectionError::InvalidState => write!(f, "the connection was in an invalid state for given operation"),
			ConnectionError::Serialization(error) => error.fmt(f),
			ConnectionError::Io(error) => error.fmt(f),
		}
	}
}

impl Error for ConnectionError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		match self {
			ConnectionError::Serialization(error) => Some(error as &dyn Error),
			ConnectionError::Io(error) => Some(error as &dyn Error),
			_ => None,
		}
	}
}

impl<T: Transmit, P: Parcel> std::fmt::Display for PendingConnectionError<T, P> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			PendingConnectionError::NoAnswer(_) => write!(f, "no answer has yet been received"),
			PendingConnectionError::InvalidAnswer(_) => write!(f, "an misformed answer has been received"),
			PendingConnectionError::Io(error) => error.fmt(f),
			PendingConnectionError::Rejected => write!(f, "connection has been actively rejected by the other end"),
			PendingConnectionError::PredicateFail => write!(f, "the answer from the other end failed the predicate"),
		}
	}
}

impl<T, P> Error for PendingConnectionError<T, P> where
	T: Transmit + std::fmt::Debug,
	P: Parcel + std::fmt::Debug,
{
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		match self {
			PendingConnectionError::Io(error) => Some(error as &dyn Error),
			_ => None,
		}
	}	
}

impl<T: Transmit, P: Parcel> From<IoError> for PendingConnectionError<T, P> {
	fn from(error: IoError) -> Self { PendingConnectionError::Io(error) }
}

impl<T: Transmit, P: Parcel> Connection<T, P> {
	const PAYLOAD_BYTE_COUNT: usize = T::PACKET_BYTE_COUNT - std::mem::size_of::<PacketHeader>();

	/// Attempt to establish a new connection to provided remote address from provided local one.
	#[inline]
	pub fn connect(endpoint: T, remote: SocketAddr, payload: Vec<u8>) -> Result<PendingConnection<T, P>, ConnectError> {
		if payload.len() > Self::PAYLOAD_BYTE_COUNT {
			Err(ConnectError::PayloadTooLarge)
		} else {
			let mut packet_buffer = Vec::with_capacity(T::PACKET_BYTE_COUNT);
			packet_buffer.resize_with(T::PACKET_BYTE_COUNT, Default::default);
			packet::write_header(&mut packet_buffer, PacketHeader::request_connection(payload.len() as u16));
			if ! payload.is_empty() {
				packet::write_data(&mut packet_buffer, &payload, 0);
			}
			endpoint.send_to(&mut packet_buffer, remote)?;
			packet_buffer.clear();
			let communication_time = Instant::now();
			Ok(PendingConnection{
				endpoint,
				remote,
				packet_buffer,
				last_sent_packet_time: communication_time,
				last_communication_time: communication_time,
				payload,
				_message_type: PhantomData,
			})
		}
	}

	/// Get the current status (state) of the `Connection`.
	#[inline]
	pub fn status(&self) -> ConnectionStatus { self.status }

	/// Checks that the [`Connection`](Self) is in [`Open`](ConnectionStatus::Open) (normal) state.
	/// 
	/// *Note: this only queries the current status of the connection, the
	/// connection may still fail after [`is_open()`](Self::is_open) returned true.*
	#[inline]
	pub fn is_open(&self) -> bool { self.status == ConnectionStatus::Open }

	/// Get the next parcel from the connection.
	/// 
	/// Includes the data prelude from the network packet the parcel was transmitted with.
	/// 
	/// Will query the socket, pop any pending network packets and finally pop a parcel.
	pub fn pop_parcel(&mut self) -> Result<(P, [u8; 4]), ConnectionError> {
		unimplemented!("Connection functionality is under development")
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
		unimplemented!("Connection functionality is under development")
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
		unimplemented!("Connection functionality is under development")
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
		unimplemented!("Connection functionality is under development")
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
	pub fn write_item_to_stream<B: ByteSerialize>(&mut self, item: &B) -> Result<usize, ConnectionError> {
		unimplemented!("Connection functionality is under development")
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
	pub fn read_from_stream(&mut self, buffer: &mut [u8]) -> Result<usize, ConnectionError> {
		unimplemented!("Connection functionality is under development")
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
	/// - Does not do synchronization that [`read_from_stream()`](Self::read_from_stream)
	/// performs, as a result there may be more bytes ready to be read than returned.
	pub fn pending_incoming_stream_bytes(&self) -> Result<usize, ConnectionError> {
		unimplemented!("Connection functionality is under development")
	}
}

impl<T: Transmit, P: Parcel> PendingConnection<T, P> {
	/// Attempt to promote the pending connection to a full Connection.
	/// 
	/// Receives any pending network packets, supplying their payload to provided predicate.
	/// If the predicate returns true promotes to full [`Connection`](Self) in
	/// [`ConnectionStatus::Open`](ConnectionStatus::Open) state.
	pub fn try_promote<F: FnOnce(&[u8]) -> bool>(mut self, predicate: F) -> Result<Connection<T, P>, PendingConnectionError<T, P>> {
		if let Err(error) = self.endpoint.recv_all(&mut self.packet_buffer, 0) {
			match error {
				TransmitError::Io(error) => return Err(PendingConnectionError::Io(error)),
				TransmitError::NoPendingPackets => (),
			}
		};
		if self.packet_buffer.is_empty() {
			Err(PendingConnectionError::NoAnswer(self))
		} else {
			let packet = &self.packet_buffer[..T::PACKET_BYTE_COUNT];
			if predicate(packet::get_data_segment(packet)) {
				let connection_id = packet::get_header(packet).connection_id;
				// Drop the first packet as it has been processed.
				self.packet_buffer.drain(..T::PACKET_BYTE_COUNT);
				Ok(Connection{
					endpoint: self.endpoint,
					remote: self.remote,
					connection_id,
					packet_buffer: self.packet_buffer,
					status: ConnectionStatus::Open,
					last_sent_packet_time: self.last_sent_packet_time,
					last_received_packet_time: Instant::now(),

					sent_packet_buffer: Vec::with_capacity(65),
					received_packet_ack_id: 0.into(),
					received_packet_ack_mask: 0,

					_message_type: self._message_type,
				})
			} else {
				Err(PendingConnectionError::PredicateFail)
			}
		}
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
		match self.endpoint.recv_all(&mut self.packet_buffer, 0) {
			Err(TransmitError::Io(error)) => return Err(PendingConnectionError::Io(error)),
			Err(TransmitError::NoPendingPackets) => (),
			Ok(_) => self.last_communication_time = Instant::now(),
		};
		if Instant::now().duration_since(self.last_communication_time) > RESYNC_PERIOD {
			// Send another request
			let original_len = self.packet_buffer.len();
			self.packet_buffer.extend(repeat(0).take(T::PACKET_BYTE_COUNT));
			let work_slice = &mut self.packet_buffer[original_len .. ];
			packet::write_header(work_slice, PacketHeader::request_connection(self.payload.len() as u16));
			if ! self.payload.is_empty() {
				packet::write_data(work_slice, &self.payload, 0);
			}
			self.endpoint.send_to(work_slice, self.remote)?;
			self.packet_buffer.truncate(original_len);
			let communication_time = Instant::now();
			self.last_communication_time = communication_time;
			self.last_sent_packet_time = communication_time;
		};
		Ok(())
	}
}
