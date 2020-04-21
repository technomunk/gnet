//! Specifics of sent udp packets.

use std::mem::size_of;
use std::num::Wrapping;
use std::cmp::{PartialOrd, Ordering};
use std::hash::Hasher;

use super::connection::ConnectionId;

pub(super) const PAYLOAD_SIZE: usize = 1024;

pub(super) const PACKET_SIZE: usize = PAYLOAD_SIZE + size_of::<PacketHeader>();

/// Networked data is preluded with this fixed-size user-data.
pub type DataPrelude = [u8; 4];

pub(super) type Hash = u32;

/// An identifying index of the packet, used to order packets.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct PacketIndex(Wrapping<u16>);

/// Header associated with each sent network packet.
#[derive(Debug, Clone, Copy, Eq)]
#[repr(C)]
pub(super) struct PacketHeader {
	pub(super) hash: Hash,
	pub(super) connection_id: ConnectionId,
	pub(super) packet_id: PacketIndex,
	/// Id of the latest acknowledged packet.
	pub(super) ack_packet_id: PacketIndex,
	/// Bitmask of 32 acks for preceding packets (32 packets before `ack_packet_id`).
	pub(super) ack_packet_mask: u32,
	/// User-provided prelude,
	pub(super) prelude: DataPrelude,
}

pub(super) type PacketBuffer = Box<[u8]>;

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
	pub(super) fn new_request_connection() -> Self {
		Self {
			hash: 0,
			connection_id: 0,
			packet_id: 1.into(),
			ack_packet_id: 0.into(),
			ack_packet_mask: 0,
			prelude: [0; 4],
		}
	}
}

/// Create a new packet-buffer.
#[inline]
pub(super) fn new_buffer() -> PacketBuffer {
	Box::new([0; PACKET_SIZE])
}

/// Get the data segment of a packet.
#[inline]
pub(super) fn get_data_segment(packet: &[u8]) -> &[u8] {
	debug_assert!(packet.len() == PACKET_SIZE);
	&packet[size_of::<PacketHeader>()..]
}

/// Get the mutable data segment of a packet.
#[inline]
pub(super) fn get_mut_data_segment(packet: &mut [u8]) -> &mut [u8] {
	debug_assert!(packet.len() == PACKET_SIZE);
	&mut packet[size_of::<PacketHeader>()..]
}

/// Get the header segment of a packet.
#[inline]
pub(super) fn get_header(packet: &[u8]) -> &PacketHeader {
	debug_assert!(packet.len() == PACKET_SIZE);
	unsafe { &*(packet.as_ptr() as *const PacketHeader) }
}

/// Write the provided data into the provided packet data segment.
#[inline]
pub(super) fn write_data(packet: &mut [u8], data: &[u8], offset: usize) {
	debug_assert!(packet.len() == PACKET_SIZE);
	debug_assert!(data.len() + offset <= PAYLOAD_SIZE);
	let offset = offset + size_of::<PacketHeader>();
	&packet[offset .. offset + data.len()].copy_from_slice(data);
}

/// Write the provided packet header into provided packet.
#[inline]
pub(super) fn write_header(packet: &mut [u8], header: PacketHeader) {
	debug_assert!(packet.len() == PACKET_SIZE);
	unsafe { *(packet.as_mut_ptr() as *mut PacketHeader) = header }
}

/// Generate the hash associated with data in provided packet.
#[inline]
pub(super) fn generate_hash<H: Hasher>(packet: &[u8], mut hasher: H) -> Hash {
	debug_assert!(packet.len() == PACKET_SIZE);
	hasher.write(&packet[size_of::<Hash>()..]);
	hasher.finish() as Hash
}

/// Generate the hash associated with data in provided packet and write it to the packet immediately.
#[inline]
pub(super) fn generate_and_write_hash<H: Hasher>(packet: &mut [u8], hasher: H) {
	debug_assert!(packet.len() == PACKET_SIZE);
	unsafe { *(packet.as_ptr() as *mut Hash) = generate_hash(packet, hasher) }
}

/// Read the hash from the packet.
#[inline]
pub(super) fn read_hash(packet: &[u8]) -> Hash {
	debug_assert!(packet.len() == PACKET_SIZE);
	unsafe { *(packet.as_ptr() as *const Hash) }
}

/// Is the given packet data valid given provided hasher?
#[inline]
pub(super) fn valid_hash<H: Hasher>(packet: &[u8], hasher: H) -> bool {
	debug_assert!(packet.len() == PACKET_SIZE);
	read_hash(packet) == generate_hash(packet, hasher)
}
