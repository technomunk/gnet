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

pub mod index {

	use std::ops::{Add, Sub};
	use std::num::Wrapping;

	/// Identifying index of the parcel.
	#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
	pub struct ParcelIndex(Wrapping<u8>);
	
	impl ParcelIndex {
		/// Get the next index.
		#[inline]
		pub fn next(self) -> Self {
			Self(self.0 + Wrapping(1))
		}
	
		/// Get the number of indices between to and from (to - from).
		#[inline]
		pub fn dist(to: Self, from: Self) -> u8 {
			(to.0 - from.0).0
		}
	
		/// Return the memory representation of this integer as a byte array in little-endian byte
		/// order.
		#[inline]
		pub fn to_le_bytes(self) -> [u8; 1] {
			self.0.0.to_le_bytes()
		}
	
		/// Construct a new integer from a byte array in little-endian byte order.
		#[inline]
		pub fn from_le_bytes(bytes: [u8; 1]) -> Self {
			Self(Wrapping(u8::from_le_bytes(bytes)))
		}
	
		/// Return the memory representation of this integer as a byte array in big-endian byte order.
		#[inline]
		pub fn to_be_bytes(self) -> [u8; 1] {
			self.0.0.to_be_bytes()
		}
	
		/// Construct a new integer from a byte array in big-endian byte order.
		#[inline]
		pub fn from_be_bytes(bytes: [u8; 1]) -> Self {
			Self(Wrapping(u8::from_be_bytes(bytes)))
		}
	}

	impl From<u8> for ParcelIndex {
		#[inline]
		fn from(idx: u8) -> Self { Self(Wrapping(idx)) }
	}

	impl Into<u8> for ParcelIndex {
		#[inline]
		fn into(self) -> u8 { self.0.0 }
	}

	impl Sub<u8> for ParcelIndex {
		type Output = Self;
		
		#[inline]
		fn sub(self, other: u8) -> Self::Output {
			Self(self.0 - Wrapping(other))
		}
	}

	impl Add<u8> for ParcelIndex {
		type Output = Self;
		
		#[inline]
		fn add(self, other: u8) -> Self::Output {
			Self(self.0 + Wrapping(other))
		}
	}
}

pub mod header;
pub mod signal;

type HandshakeId = u32;

pub use index::ParcelIndex;
pub use header::Header;
