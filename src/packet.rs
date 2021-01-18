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

use signal::*;

use super::connection::ConnectionId;

/// Networked data is preluded with this fixed-size user-data.
pub type DataPrelude = [u8; 4];

pub type Hash = u32;

/// An identifying index of the packet, used to order packets.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PacketIndex(Wrapping<u8>);

/// Protocol control bitpatterns.
mod signal {
	/// Possible signals sent in the packet protocol.
	#[derive(Debug, Clone, Copy)]
	pub enum Signal {
		/// The packet is a request for a new connection.
		// (parcel bytes == 0, stream bytes => payload size)
		ConnectionRequest,
		/// The connection is about to be closed.
		ConnectionClosed,
		/// This packet's id field is valid and should be acknowledged.
		Synchronized,
	}

	/// Compacted bitpatterns for signalling protocol-level information.
	///
	/// Consists of:
	/// | bit(s) | 31-25      | 24           | 23                | 22                 | 21-11           | 10-0         |
	/// |--------|------------|--------------|-------------------|--------------------|-----------------|--------------|
	/// | value  | `[zeroes]` | synchronized | connection_closed | connection_request | parcel(s) bytes | stream bytes |
	#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
	pub struct SignalBits(u32);

	const CONNECTION_REQUEST_BIT: u32 = 1 << 22;
	const CONNECTION_CLOSED_BIT: u32 = 1 << 23;
	const SYNCHRONIZED_BIT: u32 = 1 << 24;

	const ZERO_BITS: u32 = 0xFFFF << 25;

	const BYTE_COUNT_BITS: u32 = 0x7FF;
	const FULL_BYTE_COUNT_BITS: u32 = BYTE_COUNT_BITS << 11 | BYTE_COUNT_BITS;

	impl SignalBits {
		/// Sets the signal flags associated with given signal.
		///
		/// To read the flag use [`is_signal_set`](SignalBits::is_signal_set) method.
		#[inline]
		pub fn set_signal(&mut self, signal: Signal) {
			match signal {
				Signal::ConnectionRequest => self.0 |= CONNECTION_REQUEST_BIT,
				Signal::ConnectionClosed => self.0 |= CONNECTION_CLOSED_BIT,
				Signal::Synchronized => self.0 |= SYNCHRONIZED_BIT,
			}
		}

		/// Clears the signal flags associated with given signal.
		///
		/// To read the flag use [`is_signal_set`](SignalBits::is_signal_set) method.
		#[inline]
		pub fn clear_signal(&mut self, signal: Signal) {
			match signal {
				Signal::ConnectionRequest => self.0 &= !CONNECTION_REQUEST_BIT,
				Signal::ConnectionClosed => self.0 &= !CONNECTION_CLOSED_BIT,
				Signal::Synchronized => self.0 &= !SYNCHRONIZED_BIT,
			}
		}

		/// Checks if the signal flags associated with given signal have been set.
		///
		/// The flags are set with [`set_signal`](SignalBits::set_signal) and cleared with [`clear_signal`](SignalBits::clear_signal) methods.
		#[inline]
		pub fn is_signal_set(&self, signal: Signal) -> bool {
			match signal {
				Signal::ConnectionRequest => (self.0 & CONNECTION_REQUEST_BIT) == CONNECTION_REQUEST_BIT,
				Signal::ConnectionClosed => (self.0 & CONNECTION_CLOSED_BIT) == CONNECTION_CLOSED_BIT,
				Signal::Synchronized => (self.0 & SYNCHRONIZED_BIT) == SYNCHRONIZED_BIT,
			}
		}

		/// Set the byte-count of the parcel portion of the packet to given value.
		#[inline]
		pub fn set_parcel_byte_count(&mut self, count: u16) {
			debug_assert_eq!(count & BYTE_COUNT_BITS as u16, count);
			self.0 = (self.0 & !(BYTE_COUNT_BITS << 11)) | ((count as u32) << 11);
		}

		/// Get the byte-count of the parcel potion of the packet
		#[inline]
		pub fn get_parcel_byte_count(&self) -> u16 {
			((self.0 & (BYTE_COUNT_BITS << 11)) >> 11) as u16
		}

		/// Set the byte-count of the stream portion of the packet to given value.
		#[inline]
		pub fn set_stream_byte_count(&mut self, count: u16) {
			debug_assert_eq!(count & BYTE_COUNT_BITS as u16, count);
			self.0 = (self.0 & !BYTE_COUNT_BITS) | count as u32;
		}

		/// Get the byte-count of the stream potion of the packet
		#[inline]
		pub fn get_stream_byte_count(&self) -> u16 {
			(self.0 & BYTE_COUNT_BITS) as u16
		}

		/// Check whether the byte_count is 0 for both the stream and parcel segments.
		#[inline]
		pub fn is_empty(&self) -> bool {
			(self.0 & FULL_BYTE_COUNT_BITS) == 0
		}

		/// Create a *KeepAlive* protocol bitpattern.
		///
		/// KeepAlive packets contain no payload, they simply signal update the connection timing.
		#[inline]
		pub fn keep_alive() -> Self {
			Self(0)
		}

		/// Create a bitpattern associated with a connection request.
		#[inline]
		pub fn request_connection(payload_byte_count: u16) -> Self {
			// Since the payload length is passed from library code, this should be safe.
			debug_assert_eq!(payload_byte_count & BYTE_COUNT_BITS as u16, payload_byte_count);
			Self(CONNECTION_REQUEST_BIT | payload_byte_count as u32)
		}

		/// Create a bitpattern associated with an volatile (unsynchronized) packet with given parcel length.
		#[inline]
		pub fn volatile(parcel_byte_count: u16) -> Self {
			// Since the parcel length is passed from library code, this should be safe.
			debug_assert_eq!(parcel_byte_count & BYTE_COUNT_BITS as u16, parcel_byte_count);
			Self((parcel_byte_count as u32) << 11)
		}

		/// Create a bitpattern associated with a synchronized packet with given parcel and stream lengths.
		#[inline]
		pub fn synchronized(parcel_byte_count: u16, stream_byte_count: u16) -> Self {
			debug_assert_eq!(parcel_byte_count & BYTE_COUNT_BITS as u16, parcel_byte_count);
			debug_assert_eq!(stream_byte_count & BYTE_COUNT_BITS as u16, stream_byte_count);
			Self(SYNCHRONIZED_BIT | ((parcel_byte_count as u32) << 11) | stream_byte_count as u32)
		}

		/// Create a bitpattern associated with a packet that is informing of the connection being rejected.
		#[inline]
		pub fn reject(payload_byte_count: u16) -> Self {
			debug_assert_eq!(payload_byte_count & BYTE_COUNT_BITS as u16, payload_byte_count);
			Self(CONNECTION_CLOSED_BIT | payload_byte_count as u32)
		}

		/// Check that a given bitpattern is a valid one in GNet protocol associated with a connectionless packet.
		#[inline]
		pub fn is_valid_connectionless(&self) -> bool {
			const CRITICAL_BITS: u32 = ZERO_BITS | SYNCHRONIZED_BIT | CONNECTION_CLOSED_BIT | CONNECTION_REQUEST_BIT;
			(self.0 & CRITICAL_BITS) == CONNECTION_REQUEST_BIT
		}

		/// Check that a given bitpattern is a valid one in GNet protocol.
		#[inline]
		pub fn is_valid(&self) -> bool {
			const CRITICAL_BITS: u32 = ZERO_BITS | SYNCHRONIZED_BIT | CONNECTION_CLOSED_BIT | CONNECTION_REQUEST_BIT;
			match self.0 & CRITICAL_BITS {
				0 => true,
				SYNCHRONIZED_BIT => true,
				CONNECTION_CLOSED_BIT => true,
				CONNECTION_REQUEST_BIT => true,
				_ => false,
			}
		}
	}

	#[cfg(test)]
	mod test {
		use super::*;

		#[test]
		fn set_sizes_are_correct() {
			let mut bits = SignalBits::volatile(1024);

			assert_eq!(bits.0, 0x0020_0000);

			bits.set_stream_byte_count(11);
			bits.set_parcel_byte_count(256);

			assert_eq!(bits.0, 0x0008000B);
		}
	}
}

pub use signal::{Signal, SignalBits};

/// Header associated with each sent network packet.
#[derive(Debug, Clone, Copy, Eq)]
#[repr(C)]
pub struct PacketHeader {
	/// Id of the owning connection.
	pub connection_id: ConnectionId,
	/// Sequential index of the packet.
	pub packet_id: PacketIndex,
	/// Id of the latest acknowledged packet by the other end.
	pub ack_packet_id: PacketIndex,
	/// Bitmask of 64 acks for preceding packets (64 packets before `ack_packet_id`).
	pub ack_packet_mask: u64,
	/// Control signals for the connection.
	pub signal: SignalBits,
	/// User-provided prelude,
	pub prelude: DataPrelude,
}

impl PartialOrd for PacketIndex {
	#[inline]
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for PacketIndex {
	#[inline]
	fn cmp(&self, other: &Self) -> Ordering {
		match self.0 - other.0 {
			Wrapping(0) => Ordering::Equal,
			x if x.0 < std::u8::MAX / 2 => Ordering::Greater,
			_ => Ordering::Less,
		}
	}
}

impl From<u8> for PacketIndex {
	#[inline]
	fn from(item: u8) -> Self {
		Self(Wrapping(item))
	}
}

impl PacketIndex {
	/// Get the next index.
	#[inline]
	pub fn next(self) -> Self {
		Self(self.0 + Wrapping(1))
	}

	/// Get the number of indices between to and from (to - from).
	#[inline]
	pub fn distance(to: Self, from: Self) -> u8 {
		(to.0 - from.0).0
	}
}

impl PartialOrd for PacketHeader {
	#[inline]
	fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
		self.packet_id.partial_cmp(&rhs.packet_id)
	}
}

impl Ord for PacketHeader {
	#[inline]
	fn cmp(&self, rhs: &Self) -> Ordering {
		self.packet_id.cmp(&rhs.packet_id)
	}
}

impl PartialEq for PacketHeader {
	#[inline]
	fn eq(&self, rhs: &Self) -> bool {
		self.packet_id == rhs.packet_id
	}
}

impl PacketHeader {
	#[inline]
	fn zero() -> Self {
		Self {
			connection_id: 0,
			signal: Default::default(),
			packet_id: 0.into(),
			ack_packet_id: 0.into(),
			ack_packet_mask: 0,
			prelude: [0; 4],
		}
	}

	/// Create a packet header associated with a connection request.
	#[inline]
	pub fn request_connection(payload_byte_count: u16) -> Self {
		Self {
			signal: SignalBits::request_connection(payload_byte_count),
			.. Self::zero()
		}
	}

	/// Create a packet header for a connection-rejecting packet.
	#[inline]
	pub fn reject(payload_byte_count: u16) -> Self {
		Self {
			signal: SignalBits::reject(payload_byte_count),
			.. Self::zero()
		}
	}

	/// Check whether the header acknowledges provided packet id.
	pub fn acknowledges(&self, packet_id: PacketIndex) -> bool {
		if self.signal.is_signal_set(Signal::ConnectionRequest) {
			false
		} else {
			match PacketIndex::distance(self.ack_packet_id, packet_id) {
				0 => self.ack_packet_id == packet_id,
				x if x <= 64 => {
					let packet_bit = 1 << (x - 1);
					(self.ack_packet_mask & packet_bit) == packet_bit
				},
				_ => false,
			}
		}
	}

	/// Check that the PacketHeader is a valid GNet packet header.
	#[inline]
	pub fn is_valid(&self) -> bool {
		if self.connection_id == 0 {
			self.signal.is_valid_connectionless()
		} else {
			self.signal.is_valid()
		}
	}

	/// Get the total number of data payload bytes that the packet header accounts for.
	#[inline]
	pub fn get_payload_byte_count(&self) -> u16 {
		self.signal.get_parcel_byte_count() + self.signal.get_stream_byte_count()
	}
}

/// Get the data segment of a packet.
#[inline]
pub fn get_data_segment(packet: &[u8]) -> &[u8] {
	debug_assert!(packet.len() >= size_of::<PacketHeader>());
	&packet[size_of::<PacketHeader>() ..]
}

/// Get the valid stream portion of the packet
#[inline]
pub fn get_parcel_segment(packet: &[u8]) -> &[u8] {
	let &header = get_header(packet);
	let start = size_of::<PacketHeader>();
	let end = start + header.signal.get_parcel_byte_count() as usize;
	debug_assert!(packet.len() >= end);
	&packet[start .. end]
}

/// Get the valid stream portion of the packet
#[inline]
pub fn get_stream_segment(packet: &[u8]) -> &[u8] {
	let &header = get_header(packet);
	let start = size_of::<PacketHeader>() + header.signal.get_parcel_byte_count() as usize;
	let end = start + header.signal.get_stream_byte_count() as usize;
	debug_assert!(packet.len() >= end);
	&packet[start .. end]
}

/// Get the mutable data segment of a packet.
#[inline]
pub fn get_mut_data_segment(packet: &mut [u8]) -> &mut [u8] {
	debug_assert!(packet.len() >= size_of::<PacketHeader>());
	&mut packet[size_of::<PacketHeader>() ..]
}

/// Get the header segment of a packet.
#[inline]
pub fn get_header(packet: &[u8]) -> &PacketHeader {
	debug_assert!(packet.len() >= size_of::<PacketHeader>());
	debug_assert_eq!(packet.as_ptr().align_offset(std::mem::align_of::<PacketHeader>()), 0);
	unsafe { &*(packet.as_ptr() as *const PacketHeader) }
}

/// Write the provided data into the provided packet data segment.
#[inline]
pub fn write_data(packet: &mut [u8], data: &[u8], offset: usize) {
	debug_assert!(packet.len() >= size_of::<PacketHeader>());
	let offset = offset + size_of::<PacketHeader>();
	packet[offset..offset + data.len()].copy_from_slice(data)
}

/// Clear the remainder of the data segment of the packet starting at provided offset.
pub fn clear_remaining_data(packet: &mut [u8], offset: usize) {
	debug_assert!(packet.len() >= size_of::<PacketHeader>());
	let offset = offset + size_of::<PacketHeader>();
	for i in packet[offset..].iter_mut() {
		*i = 0
	}
}

/// Write the provided packet header into provided packet.
#[inline]
pub fn write_header(packet: &mut [u8], header: PacketHeader) {
	debug_assert!(packet.len() >= size_of::<PacketHeader>());
	debug_assert_eq!(packet.as_ptr().align_offset(std::mem::align_of::<PacketHeader>()), 0);
	unsafe { *(packet.as_mut_ptr() as *mut PacketHeader) = header }
}

/// Read the connection id from the provided packet.
pub fn read_connection_id(packet: &[u8]) -> ConnectionId {
	debug_assert!(packet.len() >= size_of::<PacketHeader>());
	get_header(packet).connection_id
}

#[inline]
pub fn is_valid(packet: &[u8]) -> bool {
	debug_assert!(packet.len() >= size_of::<PacketHeader>());
	let &header = get_header(packet);
	header.is_valid() && header.get_payload_byte_count() <= (packet.len() - size_of::<PacketHeader>()) as u16
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn packet_index_order_is_correct() {
		let smaller: PacketIndex = 0.into();
		let greater: PacketIndex = 1.into();

		assert!(smaller < greater);

		let smaller: PacketIndex = 200.into();
		let greater: PacketIndex = 1.into();

		assert!(smaller < greater);

		let smaller: PacketIndex = 60.into();
		let greater: PacketIndex = 180.into();

		assert!(smaller < greater);
	}

	#[test]
	fn packet_header_acknowledgement_is_correct() {
		let mut header = PacketHeader::request_connection(0);

		header.ack_packet_id = 17.into();
		header.ack_packet_mask = 7 << 14;

		assert_eq!(header.acknowledges(17.into()), false);

		header.signal.clear_signal(Signal::ConnectionRequest);

		assert_eq!(header.acknowledges(17.into()), true);
		assert_eq!(header.acknowledges(0.into()), true);
		assert_eq!(header.acknowledges(1.into()), true);
		assert_eq!(header.acknowledges(2.into()), true);

		assert_eq!(header.acknowledges(3.into()), false);
		assert_eq!(header.acknowledges(16.into()), false);
		assert_eq!(header.acknowledges(18.into()), false);
	}
}
