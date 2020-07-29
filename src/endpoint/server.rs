//! Server-specific endpoint implementation.

use std::net::{SocketAddr, UdpSocket};
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::iter::repeat;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::{Transmit, TransmitError};
use super::hash;

use crate::connection::ConnectionId;
use crate::packet::read_connection_id;
use crate::StableBuildHasher;

/// A trait for server-specific 'transmitters'.
/// 
/// Server 'transmitters' have the additional task of demultiplexing packets for multiple `Connections` using it.
/// The `Connections` are identified by their `ConnectionId`.
/// 
/// Additionally the server 'endpoints' may be owned by multiple connections at the same time.
pub trait ServerTransmit : Transmit {
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
/// 
/// **NOTE**: that the [`Transmit`](../trait.Transmit.html) trait is only implemented for an `Arc<Mutex<ServerUdpEndpoint>>`,
/// as the server endpoint will not function correctly otherwise.
#[derive(Debug)]
pub struct ServerUdpEndpoint<H: StableBuildHasher> {
	socket: UdpSocket,
	connections: HashMap<ConnectionId, Vec<u8>>,
	hasher_builder: H,
}

impl<H: StableBuildHasher> Transmit for Arc<Mutex<ServerUdpEndpoint<H>>> {
	// Somewhat conservative 1200 byte estimate of MTU.
	const PACKET_BYTE_COUNT: usize = 1200;

	// 4 bytes reserved for the hash
	const PACKET_HEADER_BYTE_COUNT: usize = std::mem::size_of::<hash::Hash>();

	#[inline]
	fn send_to(&self, data: &mut [u8], addr: SocketAddr) -> Result<usize, IoError> {
		let endpoint = self.lock().unwrap();
		hash::generate_and_write_hash(data, endpoint.hasher_builder.build_hasher());
		endpoint.socket.send_to(data, addr)
	}

	fn recv_all(
		&self,
		buffer: &mut Vec<u8>,
		connection_id: ConnectionId
	) -> Result<usize, TransmitError>
	{
		let mut endpoint = self.lock().unwrap();
		let original_length = buffer.len();
		buffer.extend(repeat(0).take(Self::PACKET_BYTE_COUNT));
		let work_slice = &mut buffer[original_length .. ];
		loop {
			match endpoint.socket.recv_from(work_slice) {
				Ok((packet_size, _)) => {
					if packet_size == Self::PACKET_BYTE_COUNT
					&& hash::valid_hash(work_slice, endpoint.hasher_builder.build_hasher())
					{
						// intentionally shadowed connection_id
						let connection_id = read_connection_id(work_slice);
						if let Some(buffer) = endpoint.connections.get_mut(&connection_id) {
							buffer.extend_from_slice(work_slice);
						}
					}
				},
				Err(error) => match error.kind() {
					IoErrorKind::WouldBlock => break,
					_ => return Err(TransmitError::Io(error)),
				}
			}
		};
		buffer.truncate(original_length);
		let reference_buffer = endpoint.connections.get_mut(&connection_id).unwrap();
		if reference_buffer.is_empty() {
			Err(TransmitError::NoPendingPackets)
		} else {
			buffer.extend(&reference_buffer[..]);
			let received_bytes = reference_buffer.len();
			reference_buffer.clear();
			Ok(received_bytes)
		}
	}
}

impl<H: StableBuildHasher> ServerTransmit for Arc<Mutex<ServerUdpEndpoint<H>>> {
	fn allow_connection_id(&self, connection_id: ConnectionId) {
		let mut endpoint = self.lock().unwrap();
		endpoint.connections.insert(connection_id, Vec::new());
	}

	fn block_connection_id(&self, connection_id: ConnectionId) {
		let mut endpoint = self.lock().unwrap();
		endpoint.connections.remove(&connection_id);
	}
}

impl<H:StableBuildHasher> ServerUdpEndpoint<H> {
	// TODO: query for connections?

	/// Construct a new `ServerUdpEndpoint` and bind it to provided local address.
	#[inline]
	pub fn open(addr: SocketAddr, hasher_builder: H) -> Result<Self, IoError> {
		let socket = UdpSocket::bind(addr)?;
		socket.set_nonblocking(true)?;
		Ok(Self { socket, connections: HashMap::new(), hasher_builder, })
	}
}
