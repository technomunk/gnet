//! Connections provide a continuous stream of data as long as they are valid.

use super::{ConnectError, Parcel};
use super::packet;
use super::packet::{PacketBuffer, PacketHeader};

use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::marker::PhantomData;
use std::io::{Error as IoError, ErrorKind as IoErrorKind};

/// An error specific to a pending connection.
pub enum PendingConnectionError<P: Parcel> {
	/// No answer has yet been received.
	NoAnswer(PendingConnection<P>),
	/// The answer has been received, but it was incorrect.
	InvalidAnswer(PendingConnection<P>),
	/// An unexpected IO error ocurred.
	Io((IoError, PendingConnection<P>)),
	/// The connection has been actively rejected by the other end (and subsequently consumed).
	Rejected,
}

/// A virtual connection with to remote access point.
/// 
/// This connection is not backed by a stable route (like TCP connections), however it still provides similar functionality.
pub struct Connection<P: Parcel> {
	socket: Arc<UdpSocket>,
	remote: SocketAddr,
	packet_buffer: PacketBuffer,

	_message_type: PhantomData<P>,
}

/// A temporary connection that is in the process of being established for the first time.
/// 
/// Primary purpose is to be promoted to a full connection once established or dropped on timeout.
pub struct PendingConnection<P: Parcel> {
	socket: Arc<UdpSocket>,
	remote: SocketAddr,
	packet_buffer: PacketBuffer,

	_message_type: PhantomData<P>,
}

impl<P: Parcel> Connection<P> {
	/// Attempt to establish a connection to provided remote address.
	pub fn connect(remote: SocketAddr, port: u16, payload: &[u8]) -> Result<PendingConnection<P>, ConnectError> {
		Connection::connect_with_socket(remote, Arc::new(UdpSocket::bind(("127.0.0.1", port))?), payload)
	}

	/// Attempt to establish a connection to provided remote address using an existing socket.
	pub fn connect_with_socket(remote: SocketAddr, socket: Arc<UdpSocket>, payload: &[u8]) -> Result<PendingConnection<P>, ConnectError> {
		if payload.len() > packet::PAYLOAD_SIZE {
			Err(ConnectError::PayloadTooLarge)
		} else {
			let mut packet_buffer = PacketBuffer::new();
			socket.set_nonblocking(true)?;
			socket.connect(remote);
			packet_buffer.write_header(PacketHeader::request_connection(P::PROTOCOL_ID));
			packet_buffer.write_data(payload, 0);
			socket.send_to(packet_buffer.buffer(), remote)?;
			Ok(PendingConnection{ socket, remote, packet_buffer, _message_type: PhantomData })
		}
	}
}

impl<P: Parcel> PendingConnection<P> {
	/// Check if the connection has been accepted.
	pub fn try_promote(mut self) -> Result<Connection<P>, PendingConnectionError<P>> {
		match self.socket.recv(self.packet_buffer.mut_buffer()) {
			Ok(_) => {
				let header = self.packet_buffer.read_header();
				if header.uses_protocol(P::PROTOCOL_ID) {
					// TODO: this
					Err(PendingConnectionError::NoAnswer(self))
				} else {
					Err(PendingConnectionError::InvalidAnswer(self))
				}
			},
			Err(error) => match error.kind() {
				IoErrorKind::WouldBlock => Err(PendingConnectionError::NoAnswer(self)),
				IoErrorKind::ConnectionAborted => Err(PendingConnectionError::Rejected),
				IoErrorKind::ConnectionRefused => Err(PendingConnectionError::Rejected),
				IoErrorKind::ConnectionReset => Err(PendingConnectionError::Rejected),
				_ => Err(PendingConnectionError::Io((error, self))),
			}
		}
	}
}
