//! Connections provide a continuous stream of data as long as they are valid.

use super::{ConnectError, Parcel};
use super::packet;
use super::packet::{PacketBuffer, PacketHeader};

use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::marker::PhantomData;
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::hash::{BuildHasher, Hasher};

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
	socket: Arc<UdpSocket>,
	remote: SocketAddr,
	packet_buffer: PacketBuffer,
	hash_builder: H,

	_message_type: PhantomData<P>,
}

/// A temporary connection that is in the process of being established for the first time.
/// 
/// Primary purpose is to be promoted to a full connection once established or dropped on timeout.
pub struct PendingConnection<P: Parcel, H: BuildHasher> {
	socket: Arc<UdpSocket>,
	remote: SocketAddr,
	packet_buffer: PacketBuffer,
	hash_builder: H,

	_message_type: PhantomData<P>,
}

impl<P: Parcel, H: BuildHasher> Connection<P, H> {
	/// Attempt to establish a connection to provided remote address.
	pub fn connect(remote: SocketAddr, port: u16, hash_builder: H, payload: &[u8]) -> Result<PendingConnection<P, H>, ConnectError> {
		Connection::connect_with_socket(remote, Arc::new(UdpSocket::bind(("127.0.0.1", port))?), hash_builder, payload)
	}

	/// Attempt to establish a connection to provided remote address using an existing socket.
	pub fn connect_with_socket(remote: SocketAddr, socket: Arc<UdpSocket>, hash_builder: H, payload: &[u8]) -> Result<PendingConnection<P, H>, ConnectError> {
		if payload.len() > packet::PAYLOAD_SIZE {
			Err(ConnectError::PayloadTooLarge)
		} else {
			let mut packet_buffer = PacketBuffer::new();
			socket.set_nonblocking(true)?;
			socket.connect(remote)?;
			packet_buffer.write_header(PacketHeader::new_request_connection());
			if payload.len() > 0 {
				packet_buffer.write_data(payload, 0);
			}
			packet_buffer.generate_and_write_hash(hash_builder.build_hasher());
			socket.send_to(packet_buffer.buffer(), remote)?;
			Ok(PendingConnection{ socket, remote, packet_buffer, hash_builder, _message_type: PhantomData })
		}
	}
}

impl<P: Parcel, H: BuildHasher> PendingConnection<P, H> {
	/// Check if the connection has been accepted and promote self to full connection while 
	pub fn try_promote(mut self) -> Result<Connection<P, H>, PendingConnectionError<P, H>> {
		// Loop over received messages until finding correct accept message.
		loop {
			match self.socket.recv(self.packet_buffer.mut_buffer()) {
				Ok(_) => {
					let recovered_hash = self.packet_buffer.read_hash();
					let generated_hash = self.packet_buffer.generate_hash(self.hash_builder.build_hasher());
					if recovered_hash == generated_hash {
						// TODO: success!
						return Err(PendingConnectionError::NoAnswer(self))
					};
					// continue the loop 
				},
				Err(error) => return match error.kind() {
					IoErrorKind::WouldBlock => Err(PendingConnectionError::NoAnswer(self)),
					IoErrorKind::ConnectionAborted => Err(PendingConnectionError::Rejected),
					IoErrorKind::ConnectionRefused => Err(PendingConnectionError::Rejected),
					IoErrorKind::ConnectionReset => Err(PendingConnectionError::Rejected),
					_ => Err(PendingConnectionError::Io((error, self))),
				},
			};
		}
	}
}
