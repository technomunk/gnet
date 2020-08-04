//! Client-specific endpoint trait and implementation.

use std::net::{SocketAddr, UdpSocket};
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::iter::repeat;

use super::{Transmit, TransmitError};
use super::hash;

use crate::connection::ConnectionId;
use crate::packet::read_connection_id;
use crate::StableBuildHasher;

/// Basic wrapper implementation of a ClientEndpoint over a UdpSocket.
#[derive(Debug)]
pub struct ClientUdpEndpoint<H: StableBuildHasher> {
	socket: UdpSocket,
	hasher_builder: H,
}

impl<H: StableBuildHasher> Transmit for ClientUdpEndpoint<H> {
	// Somewhat conservative 1200 byte estimate of MTU.
	const PACKET_BYTE_COUNT: usize = 1200;

	// 4 bytes reserved for the hash
	const PACKET_HEADER_BYTE_COUNT: usize = std::mem::size_of::<hash::Hash>();

	#[inline]
	fn send_to(&self, data: &mut [u8], addr: SocketAddr) -> Result<usize, IoError> {
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
			match self.socket.recv_from(&mut buffer[work_offset .. ]) {
				Ok((packet_size, _)) => {
					if packet_size == Self::PACKET_BYTE_COUNT
						&& hash::valid_hash(&buffer[work_offset .. ], self.hasher_builder.build_hasher())
						&& (connection_id == 0 || connection_id == read_connection_id(&buffer[work_offset .. ])) 
					{
						recovered_bytes += packet_size;
						work_offset = buffer.len();
						buffer.extend(repeat(0).take(Self::PACKET_BYTE_COUNT));
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

impl<H: StableBuildHasher> ClientUdpEndpoint<H> {
	/// Open a new endpoint on provided local address with provided hasher.
	/// 
	/// The hasher will be used to validate packets.
	pub fn open(addr: SocketAddr, hasher_builder: H) -> Result<Self, IoError> {
		let socket = UdpSocket::bind(addr)?;
		socket.set_nonblocking(true)?;
		Ok(Self { socket, hasher_builder })
	}
}
