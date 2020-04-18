//! Virtual connection with to remote access point.

mod packet;

use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;

use std::io::{Error as IoError};

use packet::PacketBuffer;

/// A virtual connection with to remote access point.
/// 
/// This connection is not backed by a stable route (like TCP connections), however it still provides similar functionality.
pub struct Connection {
	socket: Arc<UdpSocket>,
	remote: SocketAddr,
	packet_buffer: Box<PacketBuffer>,
}

/// A temporary connection that is in the process of being established for the first time.
/// 
/// Primary purpose is to be promoted to a full connection once established or dropped on timeout.
pub struct PendingConnection {
	socket: Arc<UdpSocket>,
	remote: SocketAddr,
	packet_buffer: Box<PacketBuffer>,
}

/// An error raised during connection process.
pub enum ConnectError {
	Io(IoError),
	PayloadTooLarge,
}

impl Connection {
	/// Attempt to establish a connection to provided remote address.
	pub fn connect(remote: SocketAddr, port: u16, payload: &[u8]) -> Result<PendingConnection, ConnectError> {
		Connection::connect_with_socket(remote, Arc::new(UdpSocket::bind(("127.0.0.1", port))?), payload)
	}

	/// Attempt to establish a connection to provided remote address using an existing socket.
	pub fn connect_with_socket(remote: SocketAddr, socket: Arc<UdpSocket>, payload: &[u8]) -> Result<PendingConnection, ConnectError> {
		if payload.len() > packet::PAYLOAD_SIZE {
			Err(ConnectError::PayloadTooLarge)
		} else {
			let mut packet_buffer = Box::new(PacketBuffer::default());
			packet_buffer.write_header();
			// TODO: fill out header with helper info
			packet_buffer.write_data(payload);
			socket.send_to(packet_buffer.as_slice(), remote)?;
			Ok(PendingConnection{ socket, remote, packet_buffer, })
		}
	}
}

impl std::convert::From<IoError> for ConnectError {
	fn from(error: IoError) -> Self {
		Self::Io(error)
	}
}
