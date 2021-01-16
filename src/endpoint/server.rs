//! Server-specific endpoint implementation.

use std::collections::{HashMap, VecDeque};
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::net::{SocketAddr, UdpSocket};
use std::sync::Mutex;

use super::hash;
use super::hash::StableBuildHasher;
use super::{Listen, Transmit, TransmitError};

use crate::connection::ConnectionId;
use crate::packet;

/// A UDP socket that caches packets for multiple connections that can be popped by
/// [`recv_all()`](Transmit::recv_all).
///
/// **NOTE**: that the [`Transmit`](Transmit) and [`Listen`](Listen) traits are only implemented
/// for [`Mutex`](Mutex)`<ServerEndpoint>`, as the server endpoint will not function correctly
/// otherwise.
#[derive(Debug)]
pub struct ServerEndpoint<H: StableBuildHasher> {
	socket: UdpSocket,
	connections: HashMap<ConnectionId, Vec<u8>>,
	hasher_builder: H,
	packet_buffer: Box<[u8]>,
	connectionless_packets: VecDeque<(SocketAddr, Box<[u8]>)>,
}

impl<H: StableBuildHasher> ServerEndpoint<H> {
	// Somewhat conservative 1200 byte estimate of MTU.
	const PACKET_BYTE_COUNT: usize = 1200;

	// 4 bytes reserved for the hash
	const RESERVED_BYTE_COUNT: usize = 8;

	fn recv_packets(&mut self) -> Result<(), IoError> {
		loop {
			match self.socket.recv_from(&mut self.packet_buffer) {
				Ok((packet_size, addr)) => {
					if packet_size == Self::PACKET_BYTE_COUNT
						&& hash::valid_hash(&self.packet_buffer, self.hasher_builder.build_hasher())
					{
						let connection_id = packet::read_connection_id(
							&self.packet_buffer[Self::RESERVED_BYTE_COUNT..],
						);
						if connection_id == 0 {
							self.connectionless_packets.push_back((addr, self.packet_buffer.clone()));
						} else if let Some(buffer) = self.connections.get_mut(&connection_id) {
							buffer.extend_from_slice(&self.packet_buffer);
						}
					}
				},
				Err(error) => match error.kind() {
					IoErrorKind::WouldBlock => break,
					_ => return Err(error),
				},
			}
		}
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

impl<H: StableBuildHasher> Transmit for Mutex<ServerEndpoint<H>> {
	// Somewhat conservative 1200 byte estimate of MTU.
	const PACKET_BYTE_COUNT: usize = ServerEndpoint::<H>::PACKET_BYTE_COUNT;

	// 4 bytes reserved for the hash
	const RESERVED_BYTE_COUNT: usize = ServerEndpoint::<H>::RESERVED_BYTE_COUNT;

	#[inline]
	fn send_to(&self, data: &mut [u8], addr: SocketAddr) -> Result<usize, IoError> {
		let endpoint = self.lock().unwrap();
		hash::generate_and_write_hash(data, endpoint.hasher_builder.build_hasher());
		endpoint.socket.send_to(data, addr)
	}

	fn recv_all(
		&self,
		buffer: &mut Vec<u8>,
		connection_id: ConnectionId,
	) -> Result<usize, TransmitError> {
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

impl<H: StableBuildHasher> Listen for Mutex<ServerEndpoint<H>> {
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

#[cfg(test)]
mod test {
	use super::hash::TestHasherBuilder;
	use super::*;
	use crate::packet;

	#[test]
	fn server_udp_sends_and_receives() {
		let a_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 1121));
		let b_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 1122));

		let a = Mutex::new(ServerEndpoint::open(a_addr, TestHasherBuilder {}).unwrap());
		a.allow_connection_id(1);
		let b = Mutex::new(ServerEndpoint::open(b_addr, TestHasherBuilder {}).unwrap());
		b.allow_connection_id(1);

		const PACKET_OFFSET: usize = ServerEndpoint::<TestHasherBuilder>::RESERVED_BYTE_COUNT;
		const PACKET_SIZE: usize = ServerEndpoint::<TestHasherBuilder>::PACKET_BYTE_COUNT;

		let mut packet_header = packet::PacketHeader {
			connection_id: 1,
			packet_id: 1.into(),
			ack_packet_id: Default::default(),
			ack_packet_mask: 0,
			signal: Default::default(),
			prelude: [ 1, 2, 3, 4, ],
		};

		let mut packet_buffer = vec![0; PACKET_SIZE];

		// Send just 1 packet

		packet::write_header(&mut packet_buffer[PACKET_OFFSET ..], packet_header);
		let send_result = a.send_to(&mut packet_buffer, b_addr);

		assert_eq!(send_result.unwrap(), PACKET_SIZE);

		packet_buffer.clear();
		let recv_result = b.recv_all(&mut packet_buffer, 1);

		assert_eq!(recv_result.unwrap(), PACKET_SIZE);
		assert_eq!(packet_buffer.len(), PACKET_SIZE);
		assert_eq!(packet_header, *packet::get_header(&packet_buffer[PACKET_OFFSET..]));

		// Send 2 packets

		packet_header.packet_id = packet_header.packet_id.next();
		packet::write_header(&mut packet_buffer[PACKET_OFFSET ..], packet_header);
		let send_result = b.send_to(&mut packet_buffer, a_addr);

		assert_eq!(send_result.unwrap(), PACKET_SIZE);

		packet_header.packet_id = packet_header.packet_id.next();
		packet::write_header(&mut packet_buffer[PACKET_OFFSET ..], packet_header);
		let send_result = b.send_to(&mut packet_buffer, a_addr);

		assert_eq!(send_result.unwrap(), PACKET_SIZE);

		let recv_result = a.recv_all(&mut packet_buffer, 1);

		assert_eq!(recv_result.unwrap(), PACKET_SIZE * 2);
		assert_eq!(packet_buffer.len(), PACKET_SIZE * 3);
		let packet_id = packet::get_header(&packet_buffer[PACKET_SIZE + PACKET_OFFSET ..]).packet_id;
		assert_eq!(packet_id, 2.into());
		let packet_id = packet::get_header(&packet_buffer[PACKET_SIZE * 2 + PACKET_OFFSET ..]).packet_id;
		assert_eq!(packet_id, 3.into());
	}

	#[test]
	fn server_udp_listens() {
		let listener_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 1123));
		let sender_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 1124));

		let listener = Mutex::new(ServerEndpoint::open(listener_addr, TestHasherBuilder {}).unwrap());
		let sender = Mutex::new(ServerEndpoint::open(sender_addr, TestHasherBuilder {}).unwrap());

		const PACKET_SIZE: usize = ServerEndpoint::<TestHasherBuilder>::PACKET_BYTE_COUNT;
		const PACKET_OFFSET: usize = ServerEndpoint::<TestHasherBuilder>::RESERVED_BYTE_COUNT;

		let packet_header = packet::PacketHeader::request_connection(4);
		let mut packet_buffer = vec![0; PACKET_SIZE];

		packet::write_header(&mut packet_buffer[PACKET_OFFSET ..], packet_header);
		packet::write_data(&mut packet_buffer[PACKET_OFFSET ..], b"GNET", 0);

		let send_result = sender.send_to(&mut packet_buffer, listener_addr);

		assert_eq!(send_result.unwrap(), PACKET_SIZE);

		let pop_result = listener.pop_connectionless_packet();

		if let Ok((addr, packet)) = pop_result {
			assert_eq!(addr, sender_addr);
			assert_eq!(&packet[..], &packet_buffer[..]);
		} else {
			panic!("No packet was popped!");
		}
	}
}
