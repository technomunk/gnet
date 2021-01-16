//! Client-specific endpoint implementation.

use std::net::{SocketAddr, UdpSocket};
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::iter::repeat;

use super::{Transmit, TransmitError};
use super::hash;
use super::hash::{StableBuildHasher};

use crate::connection::ConnectionId;
use crate::packet::read_connection_id;

/// Basic implementation of a client-side [`Endpoint`](Transmit).
///
/// Specifically contains an optimization that discards non GNet or wrongly-addressed packets.
#[derive(Debug)]
pub struct ClientEndpoint<H: StableBuildHasher> {
	socket: UdpSocket,
	hasher_builder: H,
}

impl<H: StableBuildHasher> Transmit for ClientEndpoint<H> {
	// Somewhat conservative 1200 byte estimate of MTU.
	const PACKET_BYTE_COUNT: usize = 1200;

	// 8 bytes reserved for the hash
	const RESERVED_BYTE_COUNT: usize = 8;

	#[inline]
	fn send_to(&self, data: &mut [u8], addr: SocketAddr) -> Result<usize, IoError> {
		debug_assert!(data.len() <= Self::PACKET_BYTE_COUNT);
		hash::generate_and_write_hash(data, self.hasher_builder.build_hasher());
		self.socket.send_to(data, addr)
	}

	fn recv_all(
		&self,
		buffer: &mut Vec<u8>,
		connection_id: ConnectionId,
	) -> Result<usize, TransmitError>
	{
		let mut recovered_bytes = 0;
		let mut work_offset = buffer.len();
		buffer.extend(repeat(0).take(Self::PACKET_BYTE_COUNT));
		loop {
			match self.socket.recv_from(&mut buffer[work_offset ..]) {
				Ok((packet_size, _)) => {
					let data_offset = work_offset + Self::RESERVED_BYTE_COUNT;
					if packet_size == Self::PACKET_BYTE_COUNT
						&& hash::valid_hash(&buffer[work_offset ..], self.hasher_builder.build_hasher())
						&& (connection_id == 0 || connection_id == read_connection_id(&buffer[data_offset ..])) 
					{
						recovered_bytes += packet_size;
						work_offset = buffer.len();
						buffer.extend(repeat(0).take(packet_size));
					}
					// otherwise the work_slice is reused
				},
				Err(error) => {
					// drop the work_slice
					buffer.truncate(buffer.len() - Self::PACKET_BYTE_COUNT);
					// NOTE the break!
					break match error.kind() {
						IoErrorKind::WouldBlock => if recovered_bytes > 0 {
							Ok(recovered_bytes)
						} else {
							Err(TransmitError::NoPendingPackets)
						},
						_ => Err(TransmitError::Io(error)),
					}
				}
			}
		}
	}
}

impl<H: StableBuildHasher> ClientEndpoint<H> {
	/// Open a new endpoint on provided local address with provided hasher.
	/// 
	/// The hasher will be used to validate packets.
	pub fn open(addr: SocketAddr, hasher_builder: H) -> Result<Self, IoError> {
		let socket = UdpSocket::bind(addr)?;
		socket.set_nonblocking(true)?;
		Ok(Self { socket, hasher_builder })
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use super::hash::TestHasherBuilder;
	use crate::packet;

	#[test]
	fn client_udp_sends_and_receives() {
		let a_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 1111));
		let b_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 1112));

		let a = ClientEndpoint::open(a_addr, TestHasherBuilder{}).unwrap();
		let b = ClientEndpoint::open(b_addr, TestHasherBuilder{}).unwrap();

		const PACKET_OFFSET: usize = ClientEndpoint::<TestHasherBuilder>::RESERVED_BYTE_COUNT;
		const PACKET_SIZE: usize = ClientEndpoint::<TestHasherBuilder>::PACKET_BYTE_COUNT;

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

		packet::write_header(&mut packet_buffer[PACKET_OFFSET..], packet_header);
		let send_result = a.send_to(&mut packet_buffer, b_addr);

		assert_eq!(send_result.unwrap(), PACKET_SIZE);
		
		packet_buffer.clear();
		let recv_result = b.recv_all(&mut packet_buffer, 1);

		assert_eq!(recv_result.unwrap(), PACKET_SIZE);
		assert_eq!(packet_buffer.len(), PACKET_SIZE);
		assert_eq!(packet_header, *packet::get_header(&packet_buffer[PACKET_OFFSET ..]));

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
}
