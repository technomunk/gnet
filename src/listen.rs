//! Server-side connection acceptors.
//!
//! 

#![cfg_attr(debug_assertions, allow(dead_code, unused_imports, unused_variables))]

mod accept;
// #[cfg(test)]
// pub mod test;

pub use accept::*;

use crate::connection::{Connection, ConnectionStatus,};
use crate::endpoint::{Demux, Transmit, TransmitError, Open,};
use crate::id::{ConnectionId, Allocator as ConnectionIdAllocator,};
use crate::packet;
use crate::Parcel;

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
pub struct ConnectionListener<E, P> where
	E: Transmit + Demux<ConnectionId>,
	P: Parcel,
{
	endpoint: Arc<Mutex<E>>,
	id_allocator: Mutex<ConnectionIdAllocator>,
	_message_type: PhantomData<P>,
}

impl<E, P> ConnectionListener<E, P> where
	E: Transmit + Demux<ConnectionId>,
	P: Parcel,
{
	// TODO: https://github.com/rust-lang/rust/issues/8995
	// type AcceptFn = FnOnce(SocketAddr, &[u8]) -> AcceptDecision;

	/// Construct a new listener using provided endpoint.
	pub fn new(endpoint: E) -> Self {
		Self {
			endpoint: Arc::new(Mutex::new(endpoint)),
			id_allocator: Default::default(),
			_message_type: PhantomData,
		}
	}

	/// Construct a new listener with provided wrapped endpoint.
	pub(crate) fn with_endpoint(endpoint: Arc<Mutex<E>>) -> Self {
		Self {
			endpoint,
			id_allocator: Default::default(),
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
		&self,
		predicate: F,
	) -> Result<Connection<E, P>, AcceptError> {
		todo!()
		// match self.endpoint.pop_connectionless_packet() {
		// 	Ok((address, packet)) => {
		// 		if packet::is_valid_connectionless(&packet[E::RESERVED_BYTE_COUNT ..]) {
		// 			match predicate(address, packet::get_parcel_segment(&packet[E::RESERVED_BYTE_COUNT ..])) {
		// 				AcceptDecision::Allow => {
		// 					Ok(Connection::opened(
		// 						self.endpoint.clone(),
		// 						self.id_allocator.lock().unwrap().allocate()?,
		// 						address,
		// 					))
		// 				},
		// 				AcceptDecision::Reject => {
		// 					// TODO: send reject packet
		// 					Err(AcceptError::PredicateFail)
		// 				},
		// 				AcceptDecision::Ignore => Err(AcceptError::PredicateFail),
		// 			}
		// 		} else {
		// 			Err(AcceptError::InvalidRequest(address))
		// 		}
		// 	},
		// 	Err(error) => Err(error.into()),
		// }
	}

	/// Inform the listener about a connection that was closed.
	/// 
	/// Note that the connection_id must have been assigned by the listener itself, in other
	/// words the connection closed must have come from the result of
	/// [`try_accept()`](ConnectionListener::try_accept).
	pub fn connection_closed(&self, connection_id: ConnectionId) {
		self.id_allocator.lock().unwrap().free(connection_id)
	}
}

impl<T, D, P> ConnectionListener<(T, D), P> where
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
