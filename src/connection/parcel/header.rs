//! Parcel header manipulation utilities.
//!
//! The header provides metadata about GNet parcels, such as which connections
//! they are associated with or the message content type. They also include
//! [`AckMasks`](AckMask) - mechanisms for validating received parcels.

use super::{HandshakeId, signal::Signal, index::ParcelIndex};

use crate::connection::{ack::AckMask, id::ConnectionId};

use std::convert::TryInto;

/// Parcel header.
///
/// Contains parcel metadata, such as [`ConnectionId`](ConnectionId).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
	signal: Signal,
	connection_id: ConnectionId,
	handshake_id: HandshakeId,
	index: ParcelIndex,
	ack_mask: AckMask,
	message_size: u16,
	stream_size: u16,
}

/// Error attempting to read a parcel header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
	/// The signal bitmask inside the parcel was invalid.
	///
	/// Use [`Signal::validate()`](Signal::validate) if more details are required.
	InvalidSignal,
	/// The header takes up more bytes than the provided buffer.
	///
	/// Could be the cause of corrupt signal bitmask.
	InsufficientBufferLen,
}

/// Error attempting to slice a parcel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceError {
	/// The sliced part of the parcel does not exist in the provided parcel.
	ElementDoesNotExist,
	/// The slice goes out of bounds of the provided parcel.
	///
	/// This can be caused by invalid [`Header`](Header).
	OutOfBounds,
}

/// Get the number of bytes the signal-implied header takes up in a parcel.
///
/// # Note
/// The size includes the signal byte.
fn signalled_size(signal: Signal) -> usize {
	let mut size = 1;
	// TODO: use associated constants instead of hardcoded sizes
	if signal.is_connected() {
		size += 2 + 4 * signal.is_answer() as usize;
	} else {
		size += 4;
	}
	size += signal.is_indexed() as usize;
	size += 9 * signal.has_ack_mask() as usize;
	size += 2 * signal.has_message() as usize;
	size += 2 * signal.has_stream() as usize;
	size
}

impl Header {
	/// Construct a new instance that requests a new connection.
	///
	/// # Note
	/// The provided handshake id should be unique to each connection request.
	#[inline]
	pub fn request_connection(handshake_id: HandshakeId) -> Self {
		Self {
			signal: Signal::request_connection(),
			handshake_id,
			.. Self::default()
		}
	}

	/// Construct a new instance that accepts connection request with provided handshake id.
	///
	/// Should be provided with the new connection id and the id of the request being accepted.
	#[inline]
	pub fn accept_connection(handshake_id: HandshakeId, connection_id: ConnectionId) -> Self {
		Self {
			signal: Signal::accept_connection(),
			connection_id,
			handshake_id,
			.. Self::default()
		}
	}

	/// Construct a version of the provided header that signals that the parcel contains
	/// provided number of user-app message bytes.
	#[inline]
	pub fn with_message(self, size: u16) -> Self {
		Self {
			signal: self.signal.with_message(),
			message_size: size,
			.. self
		}
	}

	/// Construct a version of the provided header that signals that the parcel contains
	/// a slice of the user-app stream. 
	pub fn with_stream(self) -> Self {
		Self {
			signal: self.signal.with_stream(),
			.. self
		}
	}

	// TODO: add more update functions

	/// Get the signal bitmask pattern of the parcel header.
	#[inline]
	pub fn signal(&self) -> Signal {
		self.signal
	}
	
	/// Get the [`ConnectionId`](ConnectionId) of the parcel.
	///
	/// Returns the id of the connection associated with the parcel.
	#[inline]
	pub fn connection_id(&self) -> Option<ConnectionId> {
		self.signal.is_connected()
			.then(|| self.connection_id)
	}

	/// Get the [`HandshakeId`](HandshakeId) of the parcel.
	///
	/// Returns the handshake id of the connection request or answer to one.
	#[inline]
	pub fn handshake_id(&self) -> Option<HandshakeId> {
		if !self.signal.is_connected() || self.signal().is_answer() {
			Some(self.handshake_id)
		} else {
			None
		}
	}

	/// Get the number of bytes of user-app message within the parcel.
	#[inline]
	pub fn message_size(&self) -> Option<u16> {
		self.signal.has_message()
			.then(|| self.message_size)
	}

	/// Get the number of bytes of user-app message within the parcel.
	#[inline]
	pub fn stream_size(&self) -> Option<u16> {
		self.signal.has_stream()
			.then(|| self.stream_size)
	}

	/// Get the number of bytes the header takes up in a parcel.
	#[inline]
	pub fn size(&self) -> usize {
		signalled_size(self.signal)
	}

	/// Get the message slice from the provided parcel assuming the header is correct.
	#[inline]
	pub fn message_slice<'a>(&self, parcel: &'a [u8]) -> Result<&'a [u8], SliceError> {
		let message_start = self.message_offset().ok_or(SliceError::ElementDoesNotExist)?;
		let message_end = message_start + self.message_size as usize;
		if message_end <= parcel.len() {
			Ok(&parcel[message_start .. message_end])
		} else {
			Err(SliceError::OutOfBounds)
		}
	}

	/// Get the mutable message slice from the provided parcel assuming the header is correct.
	#[inline]
	pub fn mut_message_slice<'a>(&self, parcel: &'a mut [u8]) -> Result<&'a mut [u8], SliceError> {
		let message_start = self.message_offset().ok_or(SliceError::ElementDoesNotExist)?;
		let message_end = message_start + self.message_size as usize;
		if message_end <= parcel.len() {
			Ok(&mut parcel[message_start .. message_end])
		} else {
			Err(SliceError::OutOfBounds)
		}
	}

	/// Get the stream slice from the provided parcel assuming the header is correct.
	#[inline]
	pub fn stream_slice<'a>(&self, parcel: &'a [u8]) -> Result<&'a [u8], SliceError> {
		let stream_start = self.stream_offset().ok_or(SliceError::ElementDoesNotExist)?;
		let stream_end = stream_start + self.message_size as usize;
		if stream_end <= parcel.len() {
			Ok(&parcel[stream_start .. stream_end])
		} else {
			Err(SliceError::OutOfBounds)
		}
	}

	/// Get the mutable stream slice from the provided parcel assuming the header is correct.
	#[inline]
	pub fn mut_stream_slice<'a>(&self, parcel: &'a mut [u8]) -> Result<&'a mut [u8], SliceError> {
		let stream_start = self.stream_offset().ok_or(SliceError::ElementDoesNotExist)?;
		let stream_end = stream_start + self.message_size as usize;
		if stream_end <= parcel.len() {
			Ok(&mut parcel[stream_start .. stream_end])
		} else {
			Err(SliceError::OutOfBounds)
		}
	}

	/// Get the offset from the beginning of the header to the beginning of the user-app
	/// message within the parcel.
	#[inline]
	pub fn message_offset(&self) -> Option<usize> {
		self.signal.has_message()
			.then(|| self.size())
	}

	/// Get the offset from the beginning of the header to the beginning of the user-app
	/// stream slice within the parcel.
	#[inline]
	pub fn stream_offset(&self) -> Option<usize> {
		self.signal.has_stream()
			.then(|| self.size() + self.message_size().unwrap_or(0) as usize)
	}

	/// Write the header to the beginning of the provided buffer.
	///
	/// # Panic
	/// Panics if the provided buffer does not fit [`size()`](Self::size) bytes.
	///
	/// # Returns
	/// Number of bytes taken up by the header.
	pub fn write_to(&self, mut buffer: &mut [u8]) -> usize {
		let size = self.size();
		assert!(buffer.len() >= size);
		buffer[0] = self.signal.into();
		buffer = &mut buffer[1..];
		if self.signal.is_connected() {
			let bytes = self.connection_id.to_le_bytes();
			buffer[.. bytes.len()].copy_from_slice(&bytes);
			buffer = &mut buffer[bytes.len() ..];
			if self.signal.is_answer() {
				let bytes = self.handshake_id.to_le_bytes();
				buffer[.. bytes.len()].copy_from_slice(&bytes);
				buffer = &mut buffer[bytes.len() ..];
			}
		} else {
			let bytes = self.handshake_id.to_le_bytes();
			buffer[.. bytes.len()].copy_from_slice(&bytes);
			buffer = &mut buffer[bytes.len() ..];
		}
		if self.signal.is_indexed() {
			buffer[0] = self.index.into();
			buffer = &mut buffer[1..];
		}
		if self.signal.has_ack_mask() {
			let bytes = self.ack_mask.to_le_bytes();
			buffer[.. bytes.len()].copy_from_slice(&bytes);
			buffer = &mut buffer[bytes.len() ..];
		}
		if self.signal.has_message() {
			let bytes = self.message_size.to_le_bytes();
			buffer[.. bytes.len()].copy_from_slice(&bytes);
			buffer = &mut buffer[bytes.len() ..];
		}
		if self.signal.has_stream() {
			let bytes = self.stream_size.to_le_bytes();
			buffer[.. bytes.len()].copy_from_slice(&bytes);
			// buffer = &mut buffer[bytes.len() ..];
		}
		size
	}

	/// Read the header from the beginning of the provided buffer.
	///
	/// # Returns
	/// The read Header and number of bytes taken up by the header.
	pub fn read_from(mut buffer: &[u8]) -> Result<(Self, usize), ReadError> {
		let signal = Signal::from(buffer[0]);
		if !signal.is_valid() {
			return Err(ReadError::InvalidSignal);
		}
		let size = signalled_size(signal);
		if buffer.len() < size {
			return Err(ReadError::InsufficientBufferLen);
		}
		buffer = &buffer[1..];

		// TODO: find out a nice way to avoid hardcoded sizes
		let mut result = Self {
			signal,
			.. Self::default()
		};
		if signal.is_connected() {
			let bytes = buffer[..2].try_into().unwrap();
			result.connection_id = ConnectionId::from_le_bytes(bytes);
			buffer = &buffer[bytes.len() ..];
			if signal.is_answer() {
				let bytes = buffer[..4].try_into().unwrap();
				result.handshake_id = HandshakeId::from_le_bytes(bytes);
				buffer = &buffer[bytes.len() ..];
			}
		} else {
			let bytes = buffer[..4].try_into().unwrap();
			result.handshake_id = HandshakeId::from_le_bytes(bytes);
			buffer = &buffer[bytes.len() ..];
		}
		if signal.is_indexed() {
			result.index = buffer[0].into();
			buffer = &buffer[1..];
		}
		if signal.has_ack_mask() {
			let bytes = buffer[..9].try_into().unwrap();
			result.ack_mask = AckMask::from_le_bytes(bytes);
			buffer = &buffer[bytes.len() ..];
		}
		if signal.has_message() {
			let bytes = buffer[..2].try_into().unwrap();
			result.message_size = u16::from_le_bytes(bytes);
			buffer = &buffer[bytes.len() ..];
		}
		if signal.has_stream() {
			let bytes = buffer[..2].try_into().unwrap();
			result.stream_size = u16::from_le_bytes(bytes);
			// buffer = &buffer[bytes.len() ..];
		}
		Ok((result, size))
	}
}

impl Default for Header {
	#[inline]
	fn default() -> Self {
		Self {
			signal: Signal::from(0),
			connection_id: 0,
			handshake_id: 0,
			index: Default::default(),
			ack_mask: AckMask::new(0.into()),
			message_size: 0,
			stream_size: 0,
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn write_read_same_header() {
		let written = Header::request_connection(12).with_message(4);
		
		let mut buffer = [0; 64];
		let written_size = written.write_to(&mut buffer);

		let (read, read_size) = Header::read_from(&buffer).unwrap();
		assert_eq!(written_size, read_size);
		assert_eq!(written, read);
	}
}
