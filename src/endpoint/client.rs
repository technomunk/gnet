//! Client-specific endpoint trait and implementation.

use std::net::{SocketAddr, UdpSocket};
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::iter::repeat;

use super::{Endpoint, EndpointError};

use crate::connection::ConnectionId;
use crate::packet;
use crate::packet::PACKET_SIZE;
use crate::StableBuildHasher;

/// A trait for a client-specific connection endpoint.
pub trait ClientEndpoint : Endpoint {}

/// Basic wrapper implementation of a ClientEndpoint over a UdpSocket.
#[derive(Debug)]
pub struct ClientUdpEndpoint {
	socket: UdpSocket,
}

impl Endpoint for ClientUdpEndpoint {
	#[inline]
	fn send_to(&self, data: &[u8], addr: SocketAddr) -> Result<usize, IoError> {
		self.socket.send_to(data, addr)
	}

	fn recv_all<H: StableBuildHasher>(
		&self,
		buffer: &mut Vec<u8>,
		connection_id: ConnectionId,
		hash_builder: &H
	) -> Result<usize, EndpointError>
	{
		let mut recovered_bytes = 0;
		let mut work_offset = buffer.len();
		buffer.extend(repeat(0).take(PACKET_SIZE));
		loop {
			match self.socket.recv_from(&mut buffer[work_offset .. ]) {
				Ok((packet_size, _)) => {
					if packet_size == PACKET_SIZE
						&& packet::valid_hash(&buffer[work_offset .. ], hash_builder.build_hasher())
						&& (connection_id == 0 || connection_id == packet::read_connection_id(&buffer[work_offset .. ])) 
					{
						recovered_bytes += packet_size;
						work_offset = buffer.len();
						buffer.extend(repeat(0).take(PACKET_SIZE));
					}
					// otherwise the work_slice is reused
				},
				Err(error) => {
					// drop the work_slice
					buffer.truncate(buffer.len() - PACKET_SIZE);
					// NOTE the break!
					break match error.kind() {
						IoErrorKind::WouldBlock => if recovered_bytes > 0 {
							Ok(recovered_bytes)
						} else {
							Err(EndpointError::NoPendingPackets)
						},
						_ => Err(EndpointError::Io(error)),
					}
				}
			}
		}
	}

	fn open(addr: SocketAddr) -> Result<Self, IoError> {
		let socket = UdpSocket::bind(addr)?;
		socket.set_nonblocking(true)?;
		Ok(Self { socket })
	}
}
