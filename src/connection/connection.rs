//! Connections provide a continuous stream of data as long as they are valid.

use super::{ConnectError, Parcel};
use super::packet;
use super::packet::{PacketBuffer, PacketHeader, PACKET_SIZE};
use super::socket::{ClientSocket, Socket, SocketError};

use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::marker::PhantomData;
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::hash::{BuildHasher, Hasher};


/// A unique index associated with a connection.
pub(super) type ConnectionId = u32;

/// An error specific to a pending connection.
pub enum PendingConnectionError<P: Parcel, H: BuildHasher> {
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

/// A virtual connection with to remote access point.
/// 
/// This connection is not backed by a stable route (like TCP connections), however it still provides similar functionality.
/// 
/// # Generic Parameters
/// 
/// - P: [Parcel](trait.Parcel.html) type of passed messages used by this `Connection`.
/// - H: [BuildHasher](trait.BuildHasher.html) the hasher used to generate a packet hash.
/// *NOTE: messages with incorrect hash are immediately discarded, meaning both ends of a connection need to have exact same `BuildHasher`.
/// It is recommended to seed the hasher with a unique secret seed for the application.*
pub struct Connection<P: Parcel, H: BuildHasher> {
	socket: Socket,
	connection_id: ConnectionId,
	remote: SocketAddr,
	hash_builder: H,
	packet_buffer: Vec<u8>,

	_message_type: PhantomData<P>,
}

/// A temporary connection that is in the process of being established for the first time.
/// 
/// Primary purpose is to be promoted to a full connection once established or dropped on timeout.
pub struct PendingConnection<P: Parcel, H: BuildHasher> {
	socket: ClientSocket,
	remote: SocketAddr,
	hash_builder: H,
	packet_buffer: Vec<u8>,

	_message_type: PhantomData<P>,
}

impl<P: Parcel, H: BuildHasher> Connection<P, H> {
	/// Attempt to establish a connection to provided remote address.
	pub fn connect(remote: SocketAddr, port: u16, hash_builder: H, payload: &[u8]) -> Result<PendingConnection<P, H>, ConnectError> {
		Connection::connect_with_socket(remote, UdpSocket::bind(("127.0.0.1", port))?, hash_builder, payload)
	}

	/// Attempt to establish a connection to provided remote address using an existing socket.
	pub fn connect_with_socket(remote: SocketAddr, socket: UdpSocket, hash_builder: H, payload: &[u8]) -> Result<PendingConnection<P, H>, ConnectError> {
		if payload.len() > packet::PAYLOAD_SIZE {
			Err(ConnectError::PayloadTooLarge)
		} else {
			let mut socket = ClientSocket::new(socket)?;
			packet::write_header(&mut socket.packet_buffer, PacketHeader::new_request_connection());
			if payload.len() > 0 {
				packet::write_data(&mut socket.packet_buffer, payload, 0);
			}
			packet::generate_and_write_hash(&mut socket.packet_buffer, hash_builder.build_hasher());
			socket.send_to(&socket.packet_buffer, remote)?;
			Ok(PendingConnection{ socket, remote, hash_builder, packet_buffer: Vec::with_capacity(PACKET_SIZE), _message_type: PhantomData })
		}
	}
}

impl<P: Parcel, H: BuildHasher> PendingConnection<P, H> {
	/// Attempt to promote the pending connection to a full Connection.
	/// 
	/// // TODO: explain the functionality and some of the necessary details 
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
				// TODO: pop the front packet
				if predicate(packet::get_data_segment(packet)) {
					let connection_id = packet::get_header(packet).connection_id;
					Ok(Connection{
						socket: Socket::Client(self.socket),
						remote: self.remote,
						hash_builder: self.hash_builder,
						connection_id,
						packet_buffer: self.packet_buffer,
						_message_type: self._message_type,
					})
				} else {
					Err(PendingConnectionError::PredicateFail)
				}
			},
		}
	}
}
