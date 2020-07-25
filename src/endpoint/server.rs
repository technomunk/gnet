//! Server-specific endpoint implementation.

use std::net::{SocketAddr, UdpSocket};
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::iter::repeat;
use std::collections::HashMap;
use std::sync::Mutex;

use super::{Endpoint, EndpointError};

use crate::connection::ConnectionId;
use crate::packet;
use crate::packet::PACKET_SIZE;
use crate::StableBuildHasher;

/// A trait for server-specific connection endpoint.
/// 
/// Servers may allow or block certain connection ids.
pub trait ServerEndpoint : Endpoint {
	/// Allow receiving packets with provided connection id.
	/// 
	/// By default all connection_ids except for `0` are assumed to be blocked.
	fn allow_connection_id(&self, connection_id: ConnectionId);

	/// Disallow receiving packets with provided connection id.
	/// 
	/// Undo `allow_connection_id`, allowing the endpoint to drop packets with provided connection id.
	/// By default all connection_ids except for `0` are assumed to be blocked.
	fn block_connection_id(&self, connection_id: ConnectionId);
}

/// A UDP socket that caches packets for multiple connections that can be popped by `recv_all()`.
#[derive(Debug)]
pub struct ServerUdpEndpoint(Mutex<ServerUdpEndpointIntern>);

#[derive(Debug)]
struct ServerUdpEndpointIntern {
	socket: UdpSocket,
	connections: HashMap<ConnectionId, Vec<u8>>,
}

impl Endpoint for ServerUdpEndpoint {
	#[inline]
	fn send_to(&self, data: &[u8], addr: SocketAddr) -> Result<usize, IoError> {
		self.0.lock().unwrap().socket.send_to(data, addr)
	}

	fn recv_all<H: StableBuildHasher>(
		&self,
		buffer: &mut Vec<u8>,
		connection_id: ConnectionId,
		hash_builder: &H
	) -> Result<usize, EndpointError>
	{
		let mut mutable_self = self.0.lock().unwrap();
		let original_length = buffer.len();
		buffer.extend(repeat(0).take(PACKET_SIZE));
		let work_slice = &mut buffer[original_length .. ];
		loop {
			match mutable_self.socket.recv_from(work_slice) {
				Ok((packet_size, _)) => {
					if packet_size == PACKET_SIZE && packet::valid_hash(work_slice, hash_builder.build_hasher())
					{
						// intentionally shadowed connection_id
						let connection_id = packet::get_header(work_slice).connection_id;
						if let Some(buffer) = mutable_self.connections.get_mut(&connection_id) {
							buffer.extend_from_slice(work_slice);
						}
					}
				},
				Err(error) => match error.kind() {
					IoErrorKind::WouldBlock => break,
					_ => return Err(EndpointError::Io(error)),
				}
			}
		};
		buffer.truncate(original_length);
		let reference_buffer = mutable_self.connections.get_mut(&connection_id).unwrap();
		if reference_buffer.is_empty() {
			Err(EndpointError::NoPendingPackets)
		} else {
			buffer.extend(&reference_buffer[..]);
			let received_bytes = reference_buffer.len();
			reference_buffer.clear();
			Ok(received_bytes)
		}
	}

	#[inline]
	fn open(addr: SocketAddr) -> Result<Self, IoError> {
		Ok(Self(Mutex::new(ServerUdpEndpointIntern::open_new(addr)?)))
	}
}

impl ServerEndpoint for ServerUdpEndpoint {
	fn allow_connection_id(&self, connection_id: ConnectionId) {
		let mut mutable_self = self.0.lock().unwrap();
		mutable_self.connections.insert(connection_id, Vec::new());
	}

	fn block_connection_id(&self, connection_id: ConnectionId) {
		let mut mutable_self = self.0.lock().unwrap();
		mutable_self.connections.remove(&connection_id);
	}
}

impl ServerUdpEndpoint {
	// TODO: query for connections?
}

impl ServerUdpEndpointIntern {
	/// Construct a new `ServerUdpEndpoint` and bind it to provided local address.
	#[inline]
	fn open_new(addr: SocketAddr) -> Result<Self, IoError> {
		let socket = UdpSocket::bind(addr)?;
		socket.set_nonblocking(true)?;
		Ok(Self { socket, connections: HashMap::new() })
	}
}
