//! Helper structs and functions to interpret and modify packet data.
//!
//! A packet is an indexable datagram sent over the network (using UDP).
//! Packets consist of 2 parts:
//! - `Header` with technical information.
//! - `Payload` with user data.
//! The payload itself may consist of:
//! - One or more instances of [`Parcel`](super::Parcel) implementations.
//! - Part of a data stream.
//!
//! The GNet uses the headers to transmit metadata, such as
//! acknowledging packets or sampling the connection latency.

use std::cmp::{Ordering, PartialOrd};
use std::mem::size_of;
use std::num::Wrapping;

/// Signalling bitpattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Signal {
	bits: u8,
}

// See docs/protocol#header
const CONNECTION_BIT: u8 = 0;
const INDEXED_BIT: u8 = 1;
const ANSWER_BIT: u8 = 1;
const ACCEPT_BIT: u8 = 2;
const ACKNOWLEDGE_BIT: u8 = 3;
const MESSAGE_BIT: u8 = 4;
const STREAM_BIT: u8 = 5;
const PARITY_BIT: u8 = 7;

impl Signal {
	/// Check whether the given signal bitpattern is valid.
	#[inline]
	pub fn is_valid(self) -> bool {
		const ZERO_MASK: u8 = 3 << 5;
		// TODO: additional checks
		(self.bits & ZERO_MASK == 0) && (self.bits.count_ones() % 2 == 1)
	}
	
	/// Check whether the parcel is associated with an established connection.
	#[inline]
	pub fn is_connected(self) -> bool {
		const MASK: u8 = 1 << CONNECTION_BIT;
		self.bits & MASK == MASK
	}

	/// Check whether the parcel is indexed.
	#[inline]
	pub fn is_indexed(self) -> bool {
		const MASK: u8 = (1 << CONNECTION_BIT) | (1 << INDEXED_BIT);
		self.bits & MASK == MASK
	}

	/// Check whether the parcel is answering a requested connection.
	#[inline]
	pub fn is_answer(self) -> bool {
		const MASK: u8 = 1 << ANSWER_BIT;
		!self.is_connected() && (self.bits & MASK == MASK)
	}

	/// Check whether the parcel is accepting a connection request.
	#[inline]
	pub fn is_accept(self) -> bool {
		const MASK: u8 = (1 << ANSWER_BIT) | (1 << ACCEPT_BIT);
		!self.is_connected() && (self.bits & MASK == MASK)
	}

	/// Check whether parcel contains an *ack mask*.
	#[inline]
	pub fn has_ack(self) -> bool {
		const MASK: u8 = (1 << CONNECTION_BIT) | (1 << ACKNOWLEDGE_BIT);
		self.bits & MASK == MASK
	}

	/// Check whether parcel contains user-application message.
	#[inline]
	pub fn has_message(self) -> bool {
		const MASK: u8 = (1 << CONNECTION_BIT) | (1 << MESSAGE_BIT);
		self.bits & MASK == MASK
	}

	/// Check whether parcel contains user-application data stream slice.
	#[inline]
	pub fn has_stream(self) -> bool {
		const MASK: u8 = (1 << CONNECTION_BIT) | (1 << INDEXED_BIT) | (1 << STREAM_BIT);
		self.bits & MASK == MASK
	}

	/// Check whether the parcel is requesting a new connection.
	#[inline]
	pub fn is_connection_request(self) -> bool {
		!self.is_connected()
	}

	/// Set the parity bit to correct state, assuming other bits do not change.
	#[inline]
	pub fn correct_parity(&mut self) {
		const PARITY_MASK: u8 = 1 << PARITY_BIT;
		if self.bits.count_ones() % 2 == 0 {
			self.bits ^= PARITY_MASK
		}
	}
}

use super::id::ConnectionId;
