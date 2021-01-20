//! Basic Transmitter implementation.

use crate::endpoint::Open;

use super::{Transmit, TransmitError};

use std::io::Error as IoError;
use std::net::{ToSocketAddrs, SocketAddr, UdpSocket};

/// Trivial implementation of [`Transmit`](Transmit) trait around [`UdpSocket`](UdpSocket).
///
/// Can be used as is or referenced for more advanced implementations.
#[derive(Debug)]
pub struct Transmitter {
	socket: UdpSocket,
}

impl Transmitter {
	/// Create a new transmitter that uses the provided socket.
	pub fn with_socket(socket: UdpSocket) -> Result<Self, IoError> {
		socket.set_nonblocking(true)?;
		Ok(Self {
			socket,
		})
	}
}

impl Transmit for Transmitter {
	// Conservative MTU approximation.
	const MAX_FRAME_LENGTH: usize = 1200;
	
	#[inline]
	fn send_to(&self, data: &[u8], addr: SocketAddr) -> Result<usize, IoError> {
		self.socket.send_to(data, addr)
	}
	
	#[inline]
	fn try_recv_from(&self, buffer: &mut [u8]) -> Result<(usize, SocketAddr), TransmitError> {
		Ok(self.socket.recv_from(buffer)?)
	}
}

impl Open for Transmitter {
	fn open<A: ToSocketAddrs>(addr: A) -> Result<Self, IoError> {
		let socket = UdpSocket::bind(addr)?;
		socket.set_nonblocking(true)?;
		Ok(Self {
			socket,
		})
	}
}

#[cfg(test)]
#[test]
fn transmitter_works() {
	let sender_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 10000));
	let sender = Transmitter::open(sender_addr).unwrap();

	let receiver_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 10001));
	let receiver = Transmitter::open(receiver_addr).unwrap();

	super::test::generic_transmit_test((&sender, sender_addr), (&receiver, receiver_addr))
}
