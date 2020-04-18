//! Virtual connection with to remote access point.

mod packet;

use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::io::{Error as IoError};
use std::marker::PhantomData;

use packet::{PacketBuffer, PacketHeader};

pub use packet::ProtocolId;

use crate::byte::ByteSerialize;

/// Possible message that is passed by connections.
pub trait Parcel : ByteSerialize {
	/// Unique identifier of the user-protocol.
	/// Network packets using a different protocol will be dropped by the connection automatically.
	const PROTOCOL_ID: ProtocolId;
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

/// An error raised during connection process.
pub enum ConnectError {
	Io(IoError),
	PayloadTooLarge,
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
			packet_buffer.write_header(PacketHeader::request_connection(P::PROTOCOL_ID));
			packet_buffer.write_data(payload, 0);
			socket.send_to(packet_buffer.buffer(), remote)?;
			Ok(PendingConnection{ socket, remote, packet_buffer, _message_type: PhantomData })
		}
	}
}

impl<P: Parcel> PendingConnection<P> {
	// TODO: promotion to full connection.
}

impl std::convert::From<IoError> for ConnectError {
	fn from(error: IoError) -> Self {
		Self::Io(error)
	}
}
