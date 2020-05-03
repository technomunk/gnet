//! Connections provide a continuous stream of data as long as they are valid.

use crate::byte::{ByteSerialize, SerializationError};

use super::{ConnectError, Parcel, StableBuildHasher};
use super::packet;
use super::packet::{PacketHeader, PACKET_SIZE};
use super::socket::{ClientSocket, Socket, SocketError};

use std::error::Error;
use std::io::{Error as IoError};
use std::marker::PhantomData;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};


/// A unique index associated with a connection.
pub(super) type ConnectionId = u32;

/// An error during the operation of a [`Connection`](struct.Connection.html).
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
pub enum PendingConnectionError<P: Parcel, H: StableBuildHasher> {
	/// No answer has yet been received.
	NoAnswer(PendingConnection<P, H>),
	/// The answer has been received, but it was incorrect.
	InvalidAnswer(PendingConnection<P, H>),
	/// An unexpected IO error ocurred.
	Io((IoError, PendingConnection<P, H>)),
	/// The connection has been actively rejected by the other end (and subsequently consumed).
	Rejected,
	/// The predicate passed to `try_promote()` returned false.
	PredicateFail,
}

/// State of a [Connection](struct.Connection.html).
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
/// This connection is not backed by a stable route (like TCP connections), however it still provides similar functionality.
/// 
/// # Generic Parameters
/// 
/// - P: [Parcel](trait.Parcel.html) type of passed messages used by this `Connection`.
/// - H: [StableBuildHasher](trait.StableBuildHasher.html) the hasher used to generate a packet hash.
/// *NOTE: messages with incorrect hash are immediately discarded, meaning both ends of a connection need to have exact same `BuildHasher`.
/// It is recommended to seed the hasher with a unique secret seed for the application.*
pub struct Connection<P: Parcel, H: StableBuildHasher> {
	socket: Socket,
	connection_id: ConnectionId,
	remote: SocketAddr,
	hash_builder: H,
	packet_buffer: Vec<u8>,
	status: ConnectionStatus,
	last_sent_packet_time: Instant,
	last_received_packet_time: Instant,

	_message_type: PhantomData<P>,
}

/// A temporary connection that is in the process of being established for the first time.
/// 
/// Primary purpose is to be promoted to a full connection once established or dropped on timeout.
#[derive(Debug)]
pub struct PendingConnection<P: Parcel, H: StableBuildHasher> {
	socket: ClientSocket,
	remote: SocketAddr,
	hash_builder: H,
	packet_buffer: Vec<u8>,
	last_sent_packet_time: Instant,

	_message_type: PhantomData<P>,
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

impl<P: Parcel, H: StableBuildHasher> Connection<P, H> {
	/// Attempt to establish a connection to provided remote address.
	#[inline]
	pub fn connect(remote: SocketAddr, port: u16, hash_builder: H, payload: &[u8]) -> Result<PendingConnection<P, H>, ConnectError> {
		Connection::connect_with_socket(remote, UdpSocket::bind(("127.0.0.1", port))?, hash_builder, payload)
	}

	/// Attempt to establish a connection to provided remote address using an existing socket.
	pub fn connect_with_socket(remote: SocketAddr, socket: UdpSocket, hash_builder: H, payload: &[u8]) -> Result<PendingConnection<P, H>, ConnectError> {
		if payload.len() > packet::PAYLOAD_SIZE {
			Err(ConnectError::PayloadTooLarge)
		} else {
			let mut socket = ClientSocket::new(socket)?;
			packet::write_header(&mut socket.packet_buffer, PacketHeader::request_connection(payload.len() as u16));
			if payload.len() > 0 {
				packet::write_data(&mut socket.packet_buffer, payload, 0);
			}
			packet::generate_and_write_hash(&mut socket.packet_buffer, hash_builder.build_hasher());
			socket.send_to(&socket.packet_buffer, remote)?;
			Ok(PendingConnection{
				socket,
				remote,
				hash_builder,
				packet_buffer: Vec::with_capacity(PACKET_SIZE),
				last_sent_packet_time: Instant::now(),
				_message_type: PhantomData,
			})
		}
	}

	/// Get the current status (state) of the `Connection`.
	#[inline]
	pub fn status(&self) -> ConnectionStatus { self.status }

	/// Checks that the `Connection` is in `Open` (normal) state.
	/// 
	/// *Note: this only queries the current status of the connection, the connection may still fail after `is_open()` returned true.*
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
	/// Reliable parcels are guaranteed to be delivered as long as the connection is in a valid state.
	/// The order of delivery is not guaranteed however, for order-dependent functionality use streams.
	/// 
	/// # Notes
	/// - May result in network packet dispatch.
	pub fn push_reliable_parcel(&mut self, parcel: P) -> Result<(), ConnectionError> {
		unimplemented!("Connection functionality is under development")
	}

	/// Begin unreliable transmission of provided parcel.
	/// 
	/// Unreliable (volatile) parcels are delivered in a best-effort manner, however no re-transmission occurs of the parcel was not received by the other end.
	/// The order of delivery is not guaranteed, for order-dependent functionality use streams.
	/// 
	/// # Notes
	/// - May result in network packet dispatch.
	pub fn push_volatile_parcel(&mut self, parcel: P) -> Result<(), ConnectionError> {
		unimplemented!("Connection functionality is under development")
	}

	/// Write a given slice of bytes to the connection stream.
	/// 
	/// # Streams
	/// Connection streams offer [TCP](https://en.wikipedia.org/wiki/Transmission_Control_Protocol)-like functionality for contiguous streams of data.
	/// Streams are transmitted with the same network packets as reliable parcels, reducing overall data duplication for lost packets.
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
	/// Connection streams offer [TCP](https://en.wikipedia.org/wiki/Transmission_Control_Protocol)-like functionality for contiguous streams of data.
	/// Streams are transmitted with the same network packets as reliable parcels, reducing overall data duplication for lost packets.
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
	/// Connection streams offer [TCP](https://en.wikipedia.org/wiki/Transmission_Control_Protocol)-like functionality for contiguous streams of data.
	/// Streams are transmitted with the same network packets as reliable parcels, reducing overall data duplication for lost packets.
	/// 
	/// # Notes
	/// - Will not read past the end of the provided buffer.
	pub fn read_from_stream(&mut self, buffer: &mut [u8]) -> Result<usize, ConnectionError> {
		unimplemented!("Connection functionality is under development")
	}

	/// Query the amount of bytes ready to be read from the incoming stream.
	/// 
	/// # Streams
	/// Connection streams offer [TCP](https://en.wikipedia.org/wiki/Transmission_Control_Protocol)-like functionality for contiguous streams of data.
	/// Streams are transmitted with the same network packets as reliable parcels, reducing overall data duplication for lost packets.
	/// 
	/// # Notes
	/// - Does not do synchronization that [`read_from_stream()`](struct.Connection.html#method.read_from_stream) performs, as a result there may be more bytes ready to be read than returned.
	pub fn pending_incoming_stream_bytes(&self) -> Result<usize, ConnectionError> {
		unimplemented!("Connection functionality is under development")
	}
}

impl<P: Parcel, H: StableBuildHasher> PendingConnection<P, H> {
	/// Attempt to promote the pending connection to a full Connection.
	/// 
	/// Receives any pending network packets, supplying their payload to provided predicate.
	/// If the predicate returns true promotes to full [`Connection`](struct.Connection.html) in [`ConnectionStatus::Open`](enum.ConnectionStatus.html) state.
	pub fn try_promote<F: FnOnce(&[u8]) -> bool>(mut self, predicate: F) -> Result<Connection<P, H>, PendingConnectionError<P, H>> {
		if let Err(error) = self.socket.recv_all(&mut self.packet_buffer, None, &self.hash_builder) {
			match error {
				SocketError::Io(error) => return Err(PendingConnectionError::Io((error, self))),
				SocketError::NoPendingPackets => (),
			}
		};
		match self.packet_buffer.is_empty() {
			true => Err(PendingConnectionError::NoAnswer(self)),
			false => {
				let packet = &self.packet_buffer[..PACKET_SIZE];
				if predicate(packet::get_data_segment(packet)) {
					let connection_id = packet::get_header(packet).connection_id;
					// Drop the first packet as it has been processed.
					self.packet_buffer.drain(..PACKET_SIZE);
					Ok(Connection{
						socket: Socket::Client(self.socket),
						remote: self.remote,
						hash_builder: self.hash_builder,
						connection_id,
						packet_buffer: self.packet_buffer,
						status: ConnectionStatus::Open,
						last_sent_packet_time: self.last_sent_packet_time,
						last_received_packet_time: Instant::now(),

						_message_type: self._message_type,
					})
				} else {
					Err(PendingConnectionError::PredicateFail)
				}
			},
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
	pub fn sync(&mut self) -> Result<(), PendingConnectionError<P, H>> {
		// TODO: implement
		Ok(())
	}
}
