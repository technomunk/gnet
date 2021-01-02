//! Server-specific endpoint implementation.

use std::net::{SocketAddr, UdpSocket};
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use super::{Transmit, TransmitError, Listen};
use super::hash;

use crate::connection::ConnectionId;
use crate::packet;
use crate::StableBuildHasher;

/// A UDP socket that caches packets for multiple connections that can be popped by `recv_all()`.
/// 
/// **NOTE**: that the [`Transmit`](super::Transmit) trait is only implemented for an
/// `Arc<Mutex<ServerUdpEndpoint>>`, as the server endpoint will not function correctly otherwise.
#[derive(Debug)]
pub struct ServerUdpEndpoint<H: StableBuildHasher> {
	socket: UdpSocket,
	connections: HashMap<ConnectionId, Vec<u8>>,
	hasher_builder: H,
	packet_buffer: Box<[u8]>,
	connectionless_packets: VecDeque<(SocketAddr, Box<[u8]>)>,
}

impl<H: StableBuildHasher> Transmit for Arc<Mutex<ServerUdpEndpoint<H>>> {
	// Somewhat conservative 1200 byte estimate of MTU.
	const PACKET_BYTE_COUNT: usize = 1200;

	// 4 bytes reserved for the hash
	const PACKET_HEADER_BYTE_COUNT: usize = 8;

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
		if let Err(error) = endpoint.recv_packets() {
			return Err(TransmitError::Io(error));
		};
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

impl<H: StableBuildHasher> Listen for Arc<Mutex<ServerUdpEndpoint<H>>> {
	fn allow_connection_id(&self, connection_id: ConnectionId) {
		let mut endpoint = self.lock().unwrap();
		endpoint.connections.insert(connection_id, Vec::new());
	}

	fn block_connection_id(&self, connection_id: ConnectionId) {
		let mut endpoint = self.lock().unwrap();
		endpoint.connections.remove(&connection_id);
	}

	fn pop_connectionless_packet(&self) -> Result<(SocketAddr, Box<[u8]>), TransmitError> {
		let mut endpoint = self.lock().unwrap();
		if let Err(error) = endpoint.recv_packets() {
			return Err(TransmitError::Io(error));
		};
		match endpoint.connectionless_packets.pop_front() {
			Some((addr, packet)) => Ok((addr, packet)),
			None => Err(TransmitError::NoPendingPackets),
		}
	}
}

impl<H:StableBuildHasher> ServerUdpEndpoint<H> {
	// TODO: query for connections?
	// Somewhat conservative 1200 byte estimate of MTU.
	const PACKET_BYTE_COUNT: usize = 1200;
	const DATA_OFFSET: usize = 8;

	fn recv_packets(&mut self) -> Result<(), IoError> {
		loop {
			match self.socket.recv_from(&mut self.packet_buffer) {
				Ok((packet_size, addr)) => {
					if packet_size == Self::PACKET_BYTE_COUNT
						&& hash::valid_hash(&self.packet_buffer[Self::DATA_OFFSET ..], self.hasher_builder.build_hasher())
					{
						let connection_id = packet::read_connection_id(&self.packet_buffer[Self::DATA_OFFSET ..]);
						if connection_id == 0 {
							self.connectionless_packets.push_back((addr, self.packet_buffer.clone()));
						} else if let Some(buffer) = self.connections.get_mut(&connection_id) {
							buffer.extend_from_slice(&self.packet_buffer);
						};
					}
				},
				Err(error) => match error.kind() {
					IoErrorKind::WouldBlock => break,
					_ => return Err(error),
				}
			}
		};
		Ok(())
	}

	/// Construct a new `ServerUdpEndpoint` and bind it to provided local address.
	#[inline]
	pub fn open(addr: SocketAddr, hasher_builder: H) -> Result<Self, IoError> {
		let socket = UdpSocket::bind(addr)?;
		socket.set_nonblocking(true)?;
		Ok(Self {
			socket,
			connections: HashMap::new(),
			packet_buffer: Box::new([0; Self::PACKET_BYTE_COUNT]),
			connectionless_packets: VecDeque::new(),
			hasher_builder,
		})
	}
}
