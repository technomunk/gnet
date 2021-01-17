//! Definition of listeners that the server uses to accept new connections.

use crate::connection::{Connection, ConnectionId, ConnectionStatus};
use crate::endpoint::{Listen, Transmit, TransmitError};
use crate::packet;
use crate::Parcel;

use std::marker::PhantomData;
use std::net::SocketAddr;
use std::time::Instant;

/// A listener passively listens for new connections.
///
/// The new connections are pending, letting the application
/// decide whether to accept a particular new connection.
pub struct ConnectionListener<E: Transmit + Listen + Clone, P: Parcel> {
	endpoint: E,
	_message_type: PhantomData<P>,
}

/// An error raised trying to accept an incoming connection.
pub enum AcceptError {
	/// Something happened attempting to read an incoming packet
	Transmit(TransmitError),
	/// The pending connection sent an invalid request packet and was dropped
	/// There may still be other connections to accept
	/// Contains the address of the source of the invalid request
	InvalidRequest(SocketAddr),
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
				if Self::is_valid_connection_request_packet(&packet) {
					match predicate(address, packet::get_stream_segment(&packet[E::RESERVED_BYTE_COUNT ..])) {
						AcceptDecision::Allow => {
							// TODO: consider sending an accept packet
							Ok(Connection::opened(
								self.endpoint.clone(),
								Default::default(), // TODO: figure out connection_id
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

	/// Check that provided packet is a valid connection-request one
	#[inline]
	fn is_valid_connection_request_packet(packet: &[u8]) -> bool {
		let header = packet::get_header(&packet[E::RESERVED_BYTE_COUNT ..]);

		packet.len() == E::PACKET_BYTE_COUNT
			&& header.connection_id == 0
			&& header.signal.is_signal_set(packet::Signal::ConnectionRequest)
			&& header.signal.is_signal_set(packet::Signal::Synchronized)
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

#[cfg(test)]
mod test {
	use std::sync::{Arc, Mutex};
	
	use super::*;

	use crate::packet;
	use crate::endpoint::{ServerEndpoint, ClientEndpoint, TestHasherBuilder};

	#[test]
	fn listener_accepts() {
		let listener_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 1211));
		let client_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 1212));

		let listener = {
			let endpoint = ServerEndpoint::open(listener_addr, TestHasherBuilder {}).unwrap();
			let endpoint = Arc::new(Mutex::new(endpoint));
			ConnectionListener::<_, ()>::new(endpoint)
		};
		let client = ClientEndpoint::open(client_addr, TestHasherBuilder {}).unwrap();

		const PACKET_SIZE: usize = 1200;
		const PACKET_OFFSET: usize = 8;

		assert_eq!(PACKET_SIZE, Arc::<Mutex<ServerEndpoint<TestHasherBuilder>>>::PACKET_BYTE_COUNT);
		assert_eq!(PACKET_SIZE, ClientEndpoint::<TestHasherBuilder>::PACKET_BYTE_COUNT);
		assert_eq!(PACKET_OFFSET, Arc::<Mutex<ServerEndpoint<TestHasherBuilder>>>::RESERVED_BYTE_COUNT);
		assert_eq!(PACKET_OFFSET, ClientEndpoint::<TestHasherBuilder>::RESERVED_BYTE_COUNT);

		let packet_header = packet::PacketHeader::request_connection(4);
		let mut packet_buffer = vec![0; PACKET_SIZE];

		packet::write_header(&mut packet_buffer[PACKET_OFFSET ..], packet_header);
		packet::write_data(&mut packet_buffer[PACKET_OFFSET ..], b"GNET", 0);

		let send_result = client.send_to(&mut packet_buffer, listener_addr);

		assert_eq!(send_result.unwrap(), PACKET_SIZE);

		let accept_result = listener.try_accept(|addr, payload| -> AcceptDecision {
			if addr == client_addr && b"GNET" == payload {
				AcceptDecision::Allow
			} else {
				AcceptDecision::Reject
			}
		});

		assert!(accept_result.is_ok());

		todo!("Send a packet through the connection")
	}

	#[test]
	fn listener_denies() {
		let listener_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 1211));
		let client_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 1212));

		let listener = {
			let endpoint = ServerEndpoint::open(listener_addr, TestHasherBuilder {}).unwrap();
			let endpoint = Arc::new(Mutex::new(endpoint));
			ConnectionListener::<_, ()>::new(endpoint)
		};
		let client = ClientEndpoint::open(client_addr, TestHasherBuilder {}).unwrap();

		const PACKET_SIZE: usize = 1200;
		const PACKET_OFFSET: usize = 8;

		assert_eq!(PACKET_SIZE, Arc::<Mutex<ServerEndpoint<TestHasherBuilder>>>::PACKET_BYTE_COUNT);
		assert_eq!(PACKET_SIZE, ClientEndpoint::<TestHasherBuilder>::PACKET_BYTE_COUNT);
		assert_eq!(PACKET_OFFSET, Arc::<Mutex<ServerEndpoint<TestHasherBuilder>>>::RESERVED_BYTE_COUNT);
		assert_eq!(PACKET_OFFSET, ClientEndpoint::<TestHasherBuilder>::RESERVED_BYTE_COUNT);

		let packet_header = packet::PacketHeader::request_connection(4);
		let mut packet_buffer = vec![0; PACKET_SIZE];

		packet::write_header(&mut packet_buffer[PACKET_OFFSET ..], packet_header);
		packet::write_data(&mut packet_buffer[PACKET_OFFSET ..], b"GNET", 0);

		let send_result = client.send_to(&mut packet_buffer, listener_addr);

		assert_eq!(send_result.unwrap(), PACKET_SIZE);

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
