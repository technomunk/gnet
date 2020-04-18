//! Specifics of sent udp packets.

use crate::byte::{ByteSerialize, SerializationError};

use std::mem::size_of;

pub(super) const PREFIX: [u8; 4] = [b'G', b'N', b'E', b'T'];
pub(super) const PAYLOAD_SIZE: usize = 1024;

#[derive(Clone)]
pub(super) struct PacketBuffer([u8; PAYLOAD_SIZE + 4]);

impl Default for PacketBuffer {
	#[inline]
	fn default() -> Self {
		Self([0; PAYLOAD_SIZE + 4])
	}
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PacketHeader {
	hash: [u8; 4],
}

impl PacketBuffer {
	#[inline]
	pub(super) fn as_slice(&self) -> &[u8] {
		&self.0
	}

	#[inline]
	pub(super) fn as_mut_slice(&mut self) -> &mut [u8] {
		&mut self.0
	}

	#[inline]
	pub(super) fn write_header(&mut self) {
		self.as_mut_slice()[..4].copy_from_slice(&PREFIX);
	}

	#[inline]
	pub(super) fn write_data(&mut self, data: &[u8]) {
		assert!(data.len() <= PAYLOAD_SIZE);
		self.as_mut_slice()[4..data.len()].copy_from_slice(data);
	}

	#[inline]
	pub(super) fn data(&self) -> &[u8] {
		&self.as_slice()[4..]
	}
}
