//! Virtual connection with to remote access point.

use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::string::{ToString};

use std::io::{Error as IoError};

/// A virtual connection with to remote access point.
/// 
/// This connection is not backed by a stable route (like TCP connections), however it still provides similar functionality.
pub struct Connection {
	socket: Arc<UdpSocket>,
	remote: SocketAddr,
}

/// A temporary connection that is in the process of being established for the first time.
/// 
/// Primary purpose is to be promoted to a full connection once established or dropped on timeout.
pub struct PendingConnection {
	socket: Arc<UdpSocket>,
	remote: SocketAddr,
}

/// An error raised during connection process.
pub enum ConnectError {
	Io(IoError),
	PayloadTooLarge,
}

impl Connection {
	/// Attempt to establish a connection to provided remote address.
	pub fn connect(remote: SocketAddr, port: u16, payload: &[u8]) -> Result<PendingConnection, ConnectError> {
		let socket = Arc::new(UdpSocket::bind(("127.0.0.1:", port))?);
		// TODO: send initial part of the handshake
		Ok(PendingConnection{ socket, remote })
	}

	/// Attempt to establish a connection to provided remote address using an existing socket.
	pub fn connect_with_socket(remote: SocketAddr, socket: Arc<UdpSocket>, payload: &[u8]) -> Result<PendingConnection, ConnectError> {
		Ok(PendingConnection{ socket, remote })
	}
}

impl std::convert::From<IoError> for ConnectError {
	fn from(error: IoError) -> Self {
		Self::Io(error)
	}
}
