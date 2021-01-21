//! Basic Transmitter implementation.

use crate::endpoint::Open;

use super::{Transmit, TransmitError};

use std::io::Error as IoError;
use std::net::{ToSocketAddrs, SocketAddr, UdpSocket};

impl Transmit for UdpSocket {
	// Conservative MTU approximation.
	const MAX_FRAME_LENGTH: usize = 1200;
	
	#[inline]
	fn send_to(&self, data: &[u8], addr: SocketAddr) -> Result<usize, IoError> {
		self.send_to(data, addr)
	}
	
	#[inline]
	fn try_recv_from(&self, buffer: &mut [u8]) -> Result<(usize, SocketAddr), TransmitError> {
		Ok(self.recv_from(buffer)?)
	}
}

impl Open for UdpSocket {
	#[inline]
	fn open<A: ToSocketAddrs>(addr: A) -> Result<Self, IoError> {
		UdpSocket::bind(addr)
	}
}

#[cfg(test)]
#[test]
fn udp_socket_transmits() {
	let sender_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 10000));
	let sender = UdpSocket::bind(sender_addr).unwrap();

	let receiver_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 10001));
	let receiver = UdpSocket::bind(receiver_addr).unwrap();

	super::test::generic_transmit_test((&sender, sender_addr), (&receiver, receiver_addr))
}
