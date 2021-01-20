//! Endpoint trait definitions, basic implementations and tests.
//!
//! An endpoint is one of the 2 ends of a connection. Basic endpoint simply send and receive
//! datagrams across network. More advanced ones are also responsible for demultiplexing datagrams
//! for multiple connections, facilitating more efficient usage of network resources.
//!
//! The library provides basic [`Transmitter`](basic::Transmitter) and
//! [`Demultiplexer`](basic::Demultiplexer) implementations, however the user may provide their own
//! implementations that will be used by GNet. It is recommended to use generic [tests](test), as they
//! test specific details that are important for correct GNet functionality.


use crate::packet;
use crate::id::ConnectionId;

use std::borrow::{Borrow, BorrowMut};
use std::io::Error as IoError;
use std::net::{ToSocketAddrs, SocketAddr};
use std::marker::PhantomData;

pub mod transmit;
pub mod demux;
#[cfg(feature = "basic-endpoints")]
pub mod basic;

pub use transmit::*;
pub use demux::*;

/// A trait for objects that may be opened on a provided address.
pub trait Open: Sized {
	/// Attempt to construct a new endpoint bound to provided address.
	fn open<A: ToSocketAddrs>(addr: A) -> Result<Self, IoError>;
}

impl<T: Transmit, D> Transmit for (T, D) {
	const MAX_FRAME_LENGTH: usize = T::MAX_FRAME_LENGTH;
	#[inline]
	fn send_to(&self, data: &[u8], addr: SocketAddr) -> Result<usize, IoError> {
		self.0.send_to(data, addr)
	}
	#[inline]
	fn try_recv_from(&self, buffer: &mut [u8]) -> Result<(usize, SocketAddr), TransmitError> {
		self.0.try_recv_from(buffer)
	}
}

impl<T, D: Demux> Demux for (T, D) {
	#[inline]
	fn allow(&mut self, connection: ConnectionId) {
		self.1.borrow_mut().allow(connection)
	}
	#[inline]
	fn block(&mut self, connection: ConnectionId) {
		self.1.borrow_mut().block(connection)
	}
	#[inline]
	fn is_allowed(&self, connection: ConnectionId) -> bool {
		self.1.borrow().is_allowed(connection)
	}
	
	#[inline]
	fn push(&mut self, connection: ConnectionId, addr: SocketAddr, datagram: &[u8]) {
		self.1.borrow_mut().push(connection, addr, datagram)
	}
	#[inline]
	fn pop(&mut self, connection: ConnectionId, buffer: &mut [u8]) -> Option<(usize, SocketAddr)> {
		self.1.borrow_mut().pop(connection, buffer)
	}
}

impl<T: Open, D: Default> Open for (T, D) {
	fn open<A: ToSocketAddrs>(addr: A) -> Result<Self, IoError> {
		Ok((T::open(addr)?, D::default()))
	}
}

/// Receive all pending packets from provided endpoint, filtering only the valid ones and
/// demultiplexing them based on internal connection id.
///
/// Fails eagerly, meaning some packets may still be pending if the result ie Err(_).
/// Returns number of successfully received packets.
pub(crate) fn recv_filter_and_demux_all<E: Transmit + Demux>(
	endpoint: &mut E,
	buffer: &mut [u8],
) -> Result<usize, TransmitError> {
	let mut count = 0;
	while let (length, source) = endpoint.try_recv_from(buffer)? {
		if packet::is_valid(buffer) {
			let packet_header = packet::get_header(buffer);
			if endpoint.is_allowed(packet_header.connection_id) {
				endpoint.push(packet_header.connection_id, source, &buffer[.. length]);
			} else {
				return Err(TransmitError::MalformedPacket)
			}
			count += 1;
		}
	};
	Ok(count)
}
