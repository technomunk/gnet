//! Definition of listeners that the server uses to accept new connections.

use crate::connection::{Connection, ConnectionStatus};
use crate::endpoint::{Listen, Transmit, TransmitError};
use crate::id::{ConnectionId, Allocator as ConnectionIdAllocator};
use crate::packet;
use crate::Parcel;

use std::marker::PhantomData;
use std::net::SocketAddr;
use std::time::Instant;
use std::sync::Mutex;

/// A listener passively listens for new connections.
///
/// The new connections are pending, letting the application
/// decide whether to accept a particular new connection.
pub struct ConnectionListener<E: Transmit + Listen + Clone, P: Parcel> {
	endpoint: E,
	id_allocator: Mutex<ConnectionIdAllocator>,
	_message_type: PhantomData<P>,
}

/// An error raised trying to accept an incoming connection.
#[derive(Debug, PartialEq)]
pub enum AcceptError {
	/// Something happened attempting to read an incoming packet
	Transmit(TransmitError),
	/// The pending connection sent an invalid request packet and was dropped
	/// There may still be other connections to accept
	/// Contains the address of the source of the invalid request
	InvalidRequest(SocketAddr),
	OutOfIds,
	/// The pending connection failed the provided predicate
	/// There may still be other connections to accept
	PredicateFail,
	/// There were no connections to accept
	NoPendingConnections,
}

/// A possible result of acceptor function.
pub enum AcceptDecision {
	/// Allow the new connection. The [`try_accept()`](ConnectionListener::try_accept)
	/// will return a new connection.
	Allow,
	/// Actively refuse the new connection, sending a packet informing the client of the decision.
	Reject,
	/// Ignore the request. The client will not be informed of the failure to connect.
	Ignore,
}

impl<E: Transmit + Listen + Clone, P: Parcel> ConnectionListener<E, P> {
	// TODO: https://github.com/rust-lang/rust/issues/8995
	// type AcceptFn = FnOnce(SocketAddr, &[u8]) -> AcceptDecision;

	/// Construct a new listener using provided endpoint.
	pub fn new(endpoint: E) -> Self {
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
		match self.endpoint.pop_connectionless_packet() {
			Ok((address, packet)) => {
				if packet::is_valid_connectionless(&packet[E::RESERVED_BYTE_COUNT ..]) {
					match predicate(address, packet::get_parcel_segment(&packet[E::RESERVED_BYTE_COUNT ..])) {
						AcceptDecision::Allow => {
							Ok(Connection::opened(
								self.endpoint.clone(),
								self.id_allocator.lock().unwrap().allocate()?,
								address,
							))
						},
						AcceptDecision::Reject => {
							// TODO: send reject packet
							Err(AcceptError::PredicateFail)
						},
						AcceptDecision::Ignore => Err(AcceptError::PredicateFail),
					}
				} else {
					Err(AcceptError::InvalidRequest(address))
				}
			},
			Err(error) => Err(error.into()),
		}
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

impl From<TransmitError> for AcceptError {
	fn from(error: TransmitError) -> Self {
		if let TransmitError::NoPendingPackets = error {
			Self::NoPendingConnections
		} else {
			Self::Transmit(error)
		}
	}
}

impl From<super::id::OutOfIdsError> for AcceptError {
	fn from(error: super::id::OutOfIdsError) -> Self {
		Self::OutOfIds
	}
}

impl std::fmt::Display for AcceptError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
		match self {
			Self::Transmit(error) => error.fmt(f),
			Self::InvalidRequest(addr) => write!(f, "got incorrect connection request from {}", addr),
			Self::OutOfIds => write!(f, "ran out of connection ids to assign"),
			Self::PredicateFail => write!(f, "connection request was denied"),
			Self::NoPendingConnections => write!(f, "no connections were requested"),
		}
	}
}

impl std::error::Error for AcceptError {}

#[cfg(test)]
pub mod test {
	use crate::packet;
	use crate::packet::DataPrelude;
	use crate::connection::{Connection, PendingConnectionError};
	use super::*;
	use std::mem::size_of;

	/// Test that a [`ConnectionListener`](ConnectionListener) is able to accept new connections
	/// using provided server and client endpoint implementations.
	pub fn test_accept<S, C>(
		(listener, listener_addr): (S, SocketAddr),
		(client, client_addr): (C, SocketAddr),
	) where
		S: Transmit + Listen + Clone,
		C: Transmit + std::fmt::Debug, 
	{
		assert_eq!(S::PACKET_BYTE_COUNT, C::PACKET_BYTE_COUNT);
		assert_eq!(S::RESERVED_BYTE_COUNT, C::RESERVED_BYTE_COUNT);
		const REQUEST_DATA: &[u8] = b"GNET CONNECTION REQUEST";

		let server = ConnectionListener::<S, ()>::new(listener);
		let client_con = Connection::<C, ()>::connect(client, listener_addr, REQUEST_DATA.to_vec())
			.expect("Failed to begin establishing client connection!");

		let accept_result = server.try_accept(|addr, payload| -> AcceptDecision {
			if addr == client_addr && payload == REQUEST_DATA {
				AcceptDecision::Allow
			} else {
				AcceptDecision::Reject
			}
		});

		accept_result.expect("Failed to accept a connection!");
		client_con.try_promote().expect("Failed to promote client connection!");
	}

	/// Test that a [`ConnectionListener`](ConnectionListener) is able to deny new connections
	/// using provided server and client endpoint implementations.
	pub fn test_deny<S, C>(
		(listener, listener_addr): (S, SocketAddr),
		(client, client_addr): (C, SocketAddr),
	) where
		S: Transmit + Listen + Clone + std::fmt::Debug,
		C: Transmit + std::fmt::Debug,
	{
		assert_eq!(S::PACKET_BYTE_COUNT, C::PACKET_BYTE_COUNT);
		assert_eq!(S::RESERVED_BYTE_COUNT, C::RESERVED_BYTE_COUNT);

		let server = ConnectionListener::<S, ()>::new(listener);
		let client_con = Connection::<C, ()>::connect(client, listener_addr, vec![])
			.expect("Failed to begin establishing client connection!");

		let accept_result = server.try_accept(|_, _| -> AcceptDecision {
			AcceptDecision::Reject
		});

		assert_eq!(accept_result, Err(AcceptError::PredicateFail));
		assert_eq!(client_con.try_promote(), Err(PendingConnectionError::Rejected));
	}
}
