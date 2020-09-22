//! A listener listens for new connections, accepting or denying them.

use crate::Parcel;
use crate::connection::{Connection, ConnectionStatus};
use crate::endpoint::{Transmit, TransmitError, Listen};
use crate::packet;
use std::net::SocketAddr;
use std::marker::PhantomData;
use std::time::Instant;

/// A listener passively listens for new connections.
/// 
/// The new connections are pending, letting the application decide whether to accept a particular new connection.
pub struct Listener<E: Transmit + Listen + Clone, P: Parcel> {
	endpoint: E,
	_message_type: PhantomData<P>,
}

/// An error raised trying to accept an incoming connection.
pub enum AcceptError {
	/// Something happened attempting to read an incoming packet
	Transmit(TransmitError),
	/// The pending connection sent an invalid request packet and was dropped
	/// However there may still be other connections to accept
	/// Contains the address of the source of the invalid request
	InvalidRequest(SocketAddr),
	/// There were no connections to accept
	NoPendingConnections,
	/// The pending connection failed the provided predicate
	PredicateFail,
}

impl<E: Transmit + Listen + Clone, P: Parcel> Listener<E, P> {
	/// Construct a new listener using provided endpoint.
	pub fn new(endpoint: E) -> Self {
		Self { endpoint, _message_type: PhantomData }
	}

	/// Attempt to accept an incoming connection using provided predicate.
	/// 
	/// Does NOT block the calling thread, returning NoPendingConnections if there are no pending connections remaining.
	pub fn try_accept<F: FnOnce(SocketAddr, &[u8]) -> bool>(&self, predicate: F) -> Result<Connection<E, P>, AcceptError> {
		match self.endpoint.pop_connectionless_packet() {
			Ok((address, packet)) => {
				if Self::is_valid_connection_request_packet(&packet) {
					if predicate(address, packet::get_stream_segment(&packet)) {
						Ok(Connection {
							endpoint: self.endpoint.clone(),
							// TODO: handle ids
							connection_id: ConnectionId,
							remote: address,
							packet_buffer: Vec::new(),
							status: ConnectionStatus::Open,
							last_sent_packet_time: Instant::now(),
							last_received_packet_time: Instant::now(),
						
							sent_packet_buffer: Vec::new(),
							received_packet_ack_id: 0.into(),
							received_packet_ack_mask: 0,
						
							_message_type: PhantomData,
						})
					} else {
						Err(AcceptError::PredicateFail)
					}
				} else {
					Err(AcceptError::InvalidRequest(address))
				}
			},
			Err(error) => Err(error.into())
		}
	}

	/// Check that provided packet is a valid connection-request one
	#[inline]
	fn is_valid_connection_request_packet(packet: &[u8]) -> bool {
		let header = packet::get_header(&packet);

		packet.len() == E::PACKET_BYTE_COUNT
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
