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

use std::borrow::{Borrow, BorrowMut};
use std::io::Error as IoError;
use std::net::{ToSocketAddrs, SocketAddr};

pub mod transmit;
pub mod demux;

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

impl<T, K, D: Demux<K>> Demux<K> for (T, D) {
	#[inline]
	fn allow(&mut self, key: K) {
		self.1.borrow_mut().allow(key)
	}
	#[inline]
	fn block(&mut self, key: K) {
		self.1.borrow_mut().block(key)
	}
	#[inline]
	fn is_allowed(&self, key: K) -> bool {
		self.1.borrow().is_allowed(key)
	}
	
	#[inline]
	fn push(&mut self, key: K, dgram: (&[u8], SocketAddr)) {
		self.1.borrow_mut().push(key, dgram)
	}
	#[inline]
	fn process<F: FnMut((&[u8], SocketAddr))>(&mut self, key: K, functor: F) {
		self.1.borrow_mut().process(key, functor);
	}
}

impl<T: Open, D: Default> Open for (T, D) {
	fn open<A: ToSocketAddrs>(addr: A) -> Result<Self, IoError> {
		Ok((T::open(addr)?, D::default()))
	}
}
