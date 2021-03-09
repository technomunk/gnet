//! Server-side connection acceptors.
//!
//! 

#![cfg_attr(debug_assertions, allow(dead_code, unused_imports, unused_variables))]

mod accept;
// #[cfg(test)]
// pub mod test;

pub use accept::*;

use crate::endpoint::{Demux, Transmit, TransmitError, Open,};

use super::connection::{Connection, ConnectionStatus};
use super::id::{ConnectionId, Allocator as ConnectionIdAllocator,};
use super::packet;
use super::Parcel;

use std::io::Error as IoError;
use std::marker::PhantomData;
use std::net::{ToSocketAddrs, SocketAddr,};
use std::time::Instant;
use std::sync::{Arc, Mutex,};

/// A listener passively listens for new connections.
///
/// The new connections are pending, letting the application
/// decide whether to accept a particular new connection.
#[derive(Debug)]
pub struct ConnectionListener<T, P> where
	T: Transmit,
	P: Parcel,
{
	endpoint: E,
	id_allocator: ConnectionIdAllocator,
	packet_buffer: Vec<u8>,
	request_packets: Vec<(usize, SocketAddr)>,
	_message_type: PhantomData<P>,
}

impl<E, P> ConnectionListener<E, P> where
	E: Transmit + Demux<ConnectionId> + Clone,
	P: Parcel,
{
	// TODO: https://github.com/rust-lang/rust/issues/8995
	// type AcceptFn = FnOnce(SocketAddr, &[u8]) -> AcceptDecision;

	/// Construct a new listener using provided endpoint.
	#[inline]
	pub fn new(endpoint: E) -> Self {
		Self {
			endpoint,
			id_allocator: Default::default(),
			packet_buffer: Vec::with_capacity(E::MAX_FRAME_LENGTH),
			request_packets: Vec::new(),
			_message_type: PhantomData,
		}
	}

	/// Attempt to accept an incoming connection using provided predicate.
	///
	/// Will pop a single connection request from the endpoint, validate the packet and
	/// invoke the predicate if the request is valid. If the predicate returns
	/// [`AcceptDecision::Allow`](AcceptDecision::Allow) the function will return a newly
	/// established [`Connection`](super::Connection), otherwise it will return
	/// [`AcceptError::PredicateFail`](AcceptError::PredicateFail).
	///
	/// ## Notes
	/// Does NOT block the calling thread, returning
	/// [`AcceptError::NoPendingConnections`](AcceptError::NoPendingConnections)
	/// if there are no pending connections remaining.
	pub fn try_accept<F: FnOnce(SocketAddr, &[u8]) -> AcceptDecision>(
		&mut self,
		predicate: F,
	) -> Result<Connection<E, P>, AcceptError> {
		if self.request_packets.is_empty() {
			self.recv_connectionless_packets()?;
			if self.request_packets.is_empty() {
				return Err(AcceptError::NoPendingConnections)
			}
		}
		let (len, src) = self.request_packets.pop().unwrap();
		let packet = &self.packet_buffer[self.packet_buffer.len() - len ..];
		match predicate(src, packet::get_parcel_segment(packet)) {
			AcceptDecision::Allow => {
				Ok(Connection::opened(
					self.endpoint.clone(),
					self.id_allocator.allocate()?,
					src,
				))
			},
			AcceptDecision::Reject => {
				todo!("Send reject packet")
			},
			AcceptDecision::Ignore => Err(AcceptError::PredicateFail),
		}
	}

	/// Inform the listener about a connection that was closed.
	/// 
	/// Note that the connection_id must have been assigned by the listener itself, in other
	/// words the connection closed must have come from the result of
	/// [`try_accept()`](ConnectionListener::try_accept).
	pub fn connection_closed(&mut self, connection_id: ConnectionId) {
		self.id_allocator.free(connection_id);
		self.endpoint.block(connection_id);
	}

	/// Receive packets on the endpoint and populate packet buffer with connectionless ones.
	fn recv_connectionless_packets(&mut self) -> Result<(), TransmitError> {
		assert!(self.request_packets.is_empty());
		self.packet_buffer.resize(E::MAX_FRAME_LENGTH, 0);
		recv_filter_and_demux_all(&mut self.endpoint, &mut self.packet_buffer)?;

		let packet_buffer = &mut self.packet_buffer;
		let request_packets = &mut self.request_packets;
		let (dgram_count, byte_count) = self.endpoint.get_buffered_counts(0);
		packet_buffer.reserve(byte_count);
		request_packets.reserve(dgram_count);
		self.endpoint.process(0, |(dgram, src)| {
			request_packets.push((dgram.len(), src));
			packet_buffer.extend_from_slice(dgram);
		});

		Ok(())
	}
}

impl<T, D, P> ConnectionListener<Arc<(T, D)>, P> where
	T: Transmit,
	D: Demux<ConnectionId>,
	P: Parcel,
{
	/// Create a new `ConnectionListener` using provided [transmitter](Transmit) and default
	/// [demultiplexer](Demux). 
	pub fn with_transmitter(transmitter: T) -> Self
	where
		D: Default,
	{
		Self::new((transmitter, D::default()))
	}

	/// Create a new `ConnectionListener` using default [transmitter](Transmit) bound to provided
	/// address and provided [demultiplexer](Demux).
	pub fn open_with_demultiplexer<A>(addr: A, demultiplexer: D) -> Result<Self, IoError>
	where
		A: ToSocketAddrs,
		T: Open,
	{
		Ok(Self::new((T::open(addr)?, demultiplexer)))
	}

	/// Create a new `ConnectionListener` using provided [transmitter](Transmit) and [demultiplexer](Demux).
	#[inline]
	pub fn with_transmitter_and_demultiplexer(transmitter: T, demultiplexer: D) -> Self {
		Self::new((transmitter, demultiplexer))
	}
}
