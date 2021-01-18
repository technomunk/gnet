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
#[derive(Debug)]
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
	use super::*;

	/// Test that a [`ConnectionListener`](ConnectionListener) is able to accept new connections
	/// using provided server and client endpoint implementations.
	pub fn generic_listener_accept_test<S: Transmit + Listen + Clone, C: Transmit>(
		(listener, listener_addr): (S, SocketAddr),
		(client, client_addr): (C, SocketAddr),
	) {
		let listener = ConnectionListener::<S, ()>::new(listener);

		assert_eq!(S::PACKET_BYTE_COUNT, C::PACKET_BYTE_COUNT);
		assert_eq!(S::RESERVED_BYTE_COUNT, C::RESERVED_BYTE_COUNT);
		const REQUEST_DATA: &[u8] = b"GNET CONNECTION REQUEST";

		let packet_header = packet::PacketHeader::request_connection(REQUEST_DATA.len() as u16);
		let mut packet_buffer = vec![0; C::PACKET_BYTE_COUNT];

		packet::write_header(&mut packet_buffer[C::RESERVED_BYTE_COUNT ..], packet_header);
		packet::write_data(&mut packet_buffer[C::RESERVED_BYTE_COUNT ..], REQUEST_DATA, 0);

		assert_eq!(client.send_to(&mut packet_buffer, listener_addr).unwrap(), S::PACKET_BYTE_COUNT);

		let accept_result = listener.try_accept(|addr, payload| -> AcceptDecision {
			if addr == client_addr && payload == REQUEST_DATA {
				AcceptDecision::Allow
			} else {
				AcceptDecision::Reject
			}
		});

		assert!(accept_result.is_ok());

		todo!("Send a packet through the connection")
	}

	/// Test that a [`ConnectionListener`](ConnectionListener) is able to deny new connections
	/// using provided server and client endpoint implementations.
	pub fn generic_listener_deny_test<S: Transmit + Listen + Clone, C: Transmit>(
		(listener, listener_addr): (S, SocketAddr),
		(client, client_addr): (C, SocketAddr),
	) {
		assert_eq!(S::PACKET_BYTE_COUNT, C::PACKET_BYTE_COUNT);
		assert_eq!(S::RESERVED_BYTE_COUNT, C::RESERVED_BYTE_COUNT);
		const REQUEST_DATA: &[u8] = b"GNET CONNECTION REQUEST";

		let listener = ConnectionListener::<S, ()>::new(listener);

		let packet_header = packet::PacketHeader::request_connection(REQUEST_DATA.len() as u16);
		let mut packet_buffer = vec![0; C::PACKET_BYTE_COUNT];

		packet::write_header(&mut packet_buffer[C::RESERVED_BYTE_COUNT ..], packet_header);
		packet::write_data(&mut packet_buffer[C::RESERVED_BYTE_COUNT ..], b"GNET", 0);

		assert_eq!(client.send_to(&mut packet_buffer, listener_addr).unwrap(), S::PACKET_BYTE_COUNT);

		let accept_result = listener.try_accept(|_, _| -> AcceptDecision {
			AcceptDecision::Reject
		});

		if let Err(AcceptError::PredicateFail) = accept_result {
		} else {
			panic!("try_accept() did not fail as expected!");
		}
		
		todo!("Check that the client endpoint receives a cancel")
	}
}
