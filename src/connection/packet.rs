//! Specifics of sent udp packets.

use std::mem::size_of;
use std::num::Wrapping;
use std::cmp::{PartialOrd, Ordering};

pub(super) const PAYLOAD_SIZE: usize = 1024;
pub(super) const CONNECTION_BITS: usize = 24;

const PACKET_SIZE: usize = PAYLOAD_SIZE + size_of::<PacketHeader>();

/// A unique identifier for user protocol.
/// Used to filter out any packets that don't include it.
/// 
/// *Note: GNet provides minimal safety guarantees, a malicious attacker can send data that looks correct to GNet.
/// It is up to the application to add further protection against invalid data.*
pub type ProtocolId = u32;

/// Networked data is preluded with this fixed-size user-data.
pub type DataPrelude = [u8; 4];

/// An identifying index of the packet, used to order packets.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct PacketIndex(Wrapping<u16>);

/// Header associated with each sent network packet.
#[derive(Debug, Clone, Copy, Eq)]
pub(super) struct PacketHeader {
	pub(super) protocol: ProtocolId,
	/// Bitfield consisting of CONNECTION_BITS connection identifier and (32 - CONNECTION_BITS) signal bits.
	pub(super) signal: u32,
	pub(super) packet_id: PacketIndex,
	/// Id of the latest acknowledged packet.
	pub(super) ack_packet_id: PacketIndex,
	/// Bitmask of 32 acks for preceding packets (32 packets before `ack_packet_id`).
	pub(super) ack_packet_mask: u32,
	/// User-provided prelude,
	pub(super) prelude: DataPrelude,
}

/// Structure for processing network packets in a safe manner.
pub(super) struct PacketBuffer {
	buffer: Box<[u8]>,
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
			x if x.0 < std::u16::MAX / 2 => Ordering::Greater,
			_ => Ordering::Less,
		}
	}
}

impl From<u16> for PacketIndex {
	#[inline]
	fn from(item: u16) -> Self {
		Self(Wrapping(item))
	}
}

impl PacketIndex {
	/// Get the next index.
	#[inline]
	pub(super) fn next(self) -> Self {
		Self(self.0 + Wrapping(1))
	}

	/// Get the number of indices between to and from (to - from).
	#[inline]
	pub(super) fn distance(to: Self, from: Self) -> u16 {
		to.0 .0 - from.0 .0
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
	pub(super) fn request_connection(protocol: ProtocolId) -> Self {
		Self {
			protocol,
			signal: 0, // TODO: figure this out
			packet_id: 1.into(),
			ack_packet_id: 0.into(),
			ack_packet_mask: 0,
			prelude: [0; 4],
		}
	}

	#[inline]
	pub(super) fn uses_protocol(&self, protocol: ProtocolId) -> bool {
		self.protocol == protocol
	}

	#[inline]
	pub(super) fn connection_id(&self) -> u32 {
		self.signal & (!0 >> CONNECTION_BITS)
	}

	#[inline]
	pub(super) fn signal(&self) -> u32 {
		self.signal >> CONNECTION_BITS
	}

	#[inline]
	pub(super) fn set_signal(&mut self, signal: u32) {
		self.signal &= !0 >> CONNECTION_BITS;
		self.signal |= signal << CONNECTION_BITS;
	}
}

impl PacketBuffer {
	#[inline]
	pub(super) fn new() -> Self {
		// TODO: research whether the value is optimized away from the stack.
		Self { buffer: Box::new([0; PACKET_SIZE]) }
	}

	/// Get the reference to the internal buffer as a slice.
	#[inline]
	pub(super) fn buffer(&self) -> &[u8] {
		&self.buffer
	}

	/// Get the slice of the data (payload) buffer.
	#[inline]
	pub(super) fn data_buffer(&self) -> &[u8] {
		&self.buffer[size_of::<PacketHeader>()..]
	}

	/// Get the mutable slice of the data (payload) buffer.
	#[inline]
	pub(super) fn write_data(&mut self, data: &[u8], offset: usize) {
		debug_assert!(data.len() + offset <= PAYLOAD_SIZE);
		let beginning = size_of::<PacketHeader>() + offset;
		(&mut self.buffer[beginning .. beginning + data.len()]).copy_from_slice(data)
	}

	/// Write the provided header into the internal buffer.
	#[inline]
	pub(super) fn write_header(&mut self, header: PacketHeader) {
		debug_assert!(self.buffer.len() >= size_of::<PacketHeader>());
		unsafe { *(self.buffer.as_mut_ptr() as *mut _) = header }
	}

	/// Read a header from the internal buffer.
	#[inline]
	pub(super) fn read_header(&self) -> PacketHeader {
		debug_assert!(self.buffer.len() >= size_of::<PacketHeader>());
		unsafe { *(self.buffer.as_ptr() as *const _) }
	}
}
