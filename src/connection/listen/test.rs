//! Listener unit tests.

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
	S: Transmit + Clone,
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
	S: Transmit + Clone + std::fmt::Debug,
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