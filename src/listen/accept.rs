//! Data structures used for processing connection requests.

use crate::id::OutOfIdsError;

use super::TransmitError;

use std::net::SocketAddr;

/// An error raised trying to accept an incoming connection.
#[derive(Debug, PartialEq)]
pub enum AcceptError {
	/// Something happened attempting to read an incoming packet
	Transmit(TransmitError),
	/// The pending connection sent an invalid request packet and was dropped
	/// There may still be other connections to accept
	/// Contains the address of the source of the invalid request
	InvalidRequest(SocketAddr),
	/// There are no more connection ids to assign.
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

impl From<TransmitError> for AcceptError {
	fn from(error: TransmitError) -> Self {
		if let TransmitError::NoPendingPackets = error {
			Self::NoPendingConnections
		} else {
			Self::Transmit(error)
		}
	}
}

impl From<OutOfIdsError> for AcceptError {
	fn from(err: OutOfIdsError) -> Self {
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
