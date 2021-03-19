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

pub mod signal {
	/// Signalling bitpattern.
	#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
	pub struct Signal {
		bits: u8,
	}

	/// Error when validating a Signal bitpattern.
	#[derive(Debug, Clone, Copy, PartialEq, Eq)]
	pub enum SignalValidationError {
		/// [**Index**](Signal::is_indexed) bit was set without
		/// [**connection**](Signal::is_connected) bit being set.
		DisconnectedIndex,
		/// [**Acknowledge**](Signal::has_ack) bit was set without
		/// [**connection**](Signal::is_connected) bit being set.
		DisconnectedAcknowledge,
		/// [**Stream**](Signal::has_stream) bit was set without
		/// [**connection**](Signal::is_connected) bit being set.
		DisconnectedStream,
		/// [**Stream**](Signal::has_stream) bit was set without
		/// [**index**](Signal::is_indexed) bit being set.
		UnreliableStream,
		/// The bitpattern has even parity.
		InvalidParity,
	}

	// See docs/protocol#header
	const CONNECTION_MASK: u8  = 1 << 0;
	const ANSWER_MASK: u8      = 1 << 1;
	const RESERVED_MASK: u8    = 1 << 2;
	const INDEX_MASK: u8       = 1 << 3;
	const ACKNOWLEDGE_MASK: u8 = 1 << 4;
	const MESSAGE_MASK: u8     = 1 << 5;
	const STREAM_MASK: u8      = 1 << 6;
	const PARITY_MASK: u8      = 1 << 7;
	
	impl Signal {
		/// Construct a new instance that signals a request for a connection.
		///
		/// # Note
		/// The **signal** should be immediately followed by 4-byte randomly generated
		/// *handshake id* in the parcel.
		#[inline]
		pub const fn request_connection() -> Self {
			Self { bits: PARITY_MASK }
		}

		/// Construct a new instance that signals that the requested connection was accepted.
		///
		/// # Note
		/// The **signal** should be immediately followed by [`ConnectionId`](ConnectionId) and
		/// 4-byte *handshake id* of the accepted request.
		///
		/// Once accepted the server considers the new connection valid, it is recommended to start
		/// sending data with the *accept* parcel.
		#[inline]
		pub const fn accept_connection() -> Self {
			Self { bits: CONNECTION_MASK | ANSWER_MASK | PARITY_MASK }
		}

		/// Construct a new instance that signals that the requested connection was rejected.
		///
		/// # Notes
		/// The **signal** should be immediately followed by a 4-byte *handshake id* that is the
		/// same as in the connection-requesting parcel.
		///
		/// Invalid connection requests may also simply be ignored by the server.
		#[inline]
		pub const fn reject_connection() -> Self {
			Self { bits: ANSWER_MASK }
		}

		/// Construct a new instance that signals that the containing parcel is associated
		/// with an existent [connection](crate::connection).
		///
		/// # Notes
		/// The **signal** should be immediately followed by [`ConnectionId`] in the parcel.
		///
		/// On its own the parcel is unreliable and serves little purpose. It is recommended
		/// to also add [`AckMask`](super::AckMask) and *message* to the parcel.
		#[inline]
		pub const fn connected() -> Self {
			Self { bits: CONNECTION_MASK }
		}

		/// Check that the bitpattern is valid.
		///
		/// # Note
		/// Returns a particular validation error for invalid signals. If a simple validity check
		/// is required use [`is_valid`](Self::is_valid) instead.
		pub const fn validate(self) -> Result<(), SignalValidationError> {
			if self.is_connected() {
				if self.has_stream() && !self.is_indexed() {
					return Err(SignalValidationError::UnreliableStream)
				}
			} else {
				if self.is_indexed() {
					return Err(SignalValidationError::DisconnectedIndex)
				}
				if self.has_ack_mask() {
					return Err(SignalValidationError::DisconnectedAcknowledge)
				}
				if self.has_stream() {
					return Err(SignalValidationError::DisconnectedStream)
				}
			}
			if !self.has_correct_parity() {
				return Err(SignalValidationError::InvalidParity)
			}
			Ok(())
		}

		/// Check whether the given signal bitpattern is valid.
		///
		/// # Note
		/// Received GNet parcels with invalid **signals** should be dropped. If the error was
		/// caused by a random bit flip the library provides data reliability that will ensure
		/// the dropped data is delivered.
		#[inline]
		pub fn is_valid(self) -> bool {
			self.validate() == Ok(())
		}
		
		/// Check whether the parcel is associated with an established connection.
		///
		/// If true the parcel is associated with a [*connection*](crate::connection) and the
		/// **signal** should be immediately followed by [`ConnectionId`](ConnectionId) in the
		/// parcel.
		#[inline]
		pub const fn is_connected(self) -> bool {
			self.bits & CONNECTION_MASK == CONNECTION_MASK
		}

		/// Check whether the parcel is answering a requested connection.
		///
		/// # Note
		/// Answer on its own does not mean that the requested connection was accepted. Accepting
		/// answer is a [*connected*](Self::is_connected) one.
		#[inline]
		pub const fn is_answer(self) -> bool {
			self.bits & ANSWER_MASK == ANSWER_MASK
		}

		/// Check whether the parcel is indexed.
		///
		/// # Notes
		/// Indexed parcels must be [*connected*](Self::is_connected).
		///
		/// If true the parcel has an [`ParcelIndex`](ParcelIndex) and should be acknowledged.
		/// Unacknowledged indexed parcels are to be retransmitted. 
		#[inline]
		pub const fn is_indexed(self) -> bool {
			const MASK: u8 = CONNECTION_MASK | INDEX_MASK;
			self.bits & MASK == MASK
		}

		/// Signal that the parcel is indexed and should be acknowledged.
		/// 
		/// # Note
		/// Indexed parcels must be [*connected*](Self::is_connected). The functions does NOT
		/// update the [`connection`](Self::is_connected) bit.
		#[inline]
		pub fn indexed(self) -> Self {
			// TODO/https://github.com/rust-lang/rust/issues/51999 : mark const
			debug_assert!(self.is_connected(), "Indexed parcels must be connected!");
			Self { bits: self.bits | INDEX_MASK }.with_correct_parity()
		}

		/// Check whether parcel contains an [`AckMask`](super::AckMask).
		#[inline]
		pub const fn has_ack_mask(self) -> bool {
			const MASK: u8 = CONNECTION_MASK | ACKNOWLEDGE_MASK;
			self.bits & MASK == MASK
		}

		/// Signal that the parcel contains an [`AckMask`](super::AckMask).
		///
		/// # Note
		/// Parcels with [`AckMasks`](super::AckMask) must be [*connected*](Self::is_connected).
		/// The functions does NOT update the [`connection`](Self::is_connected) bit.
		#[inline]
		pub fn with_ack_mask(self) -> Self {
			// TODO/https://github.com/rust-lang/rust/issues/51999 : mark const
			debug_assert!(self.is_connected(), "Parcels with AckMasks must be connected!");
			Self { bits: self.bits | ACKNOWLEDGE_MASK }.with_correct_parity()
		}

		/// Check whether parcel contains user-application message bytes.
		#[inline]
		pub const fn has_message(self) -> bool {
			self.bits & MESSAGE_MASK == MESSAGE_MASK
		}

		/// Signal that the parcel contains user-application message bytes.
		#[inline]
		pub const fn with_message(self) -> Self {
			Self { bits: self.bits | MESSAGE_MASK }.with_correct_parity()
		}

		/// Check whether parcel contains user-application data stream slice.
		///
		/// # Note
		/// Stream serialization requires the parcel to be [*connected*](Self::is_connected) and
		/// [*indexed*](Self::is_indexed).
		#[inline]
		pub const fn has_stream(self) -> bool {
			const MASK: u8 = CONNECTION_MASK | INDEX_MASK | STREAM_MASK;
			self.bits & MASK == MASK
		}

		/// Signal that the parcel contains user-application data stream slice.
		///
		/// # Notes
		/// Stream serialization requires the parcel to be [*connected*](Self::is_connected) and
		/// [*indexed*](Self::is_indexed). This function does NOT set the relevant signal bits as
		/// both parameters require the signal to be followed by additional header data in the
		/// parcel.
		pub fn with_stream(self) -> Self {
			// TODO/https://github.com/rust-lang/rust/issues/51999 : mark const
			debug_assert!(self.is_connected(), "Stream parcels must be connected!");
			debug_assert!(self.is_indexed(), "Stream parcels must be reliable (indexed)!");
			Self { bits: self.bits | ACKNOWLEDGE_MASK }.with_correct_parity()
		}

		/// Check whether the signal is of odd parity.
		#[inline]
		pub const fn has_correct_parity(self) -> bool {
			self.bits.count_ones() & 1 == 1
		}

		/// Set the parity bit to correct state, assuming other bits do not change.
		#[inline]
		pub const fn with_correct_parity(self) -> Self {
			if self.has_correct_parity() {
				Self { bits: self.bits }
			} else {
				Self { bits: self.bits ^ PARITY_MASK }
			}
		}

		/// Check whether the parcel is requesting a new connection.
		#[inline]
		pub const fn is_connection_request(self) -> bool {
			const MASK: u8 = CONNECTION_MASK | ANSWER_MASK | INDEX_MASK | STREAM_MASK;
			self.bits & MASK == 0
		}

		/// Check whether the parcel is accepting a connection request.
		#[inline]
		pub const fn is_accept(self) -> bool {
			const MASK: u8 = CONNECTION_MASK | ANSWER_MASK;
			self.bits & MASK == MASK
		}
	}

	impl From<u8> for Signal {
		#[inline]
		fn from(bits: u8) -> Self {
			Self { bits }
		}
	}

	impl Into<u8> for Signal {
		#[inline]
		fn into(self) -> u8 {
			self.bits
		}
	}

	#[cfg(test)]
	mod test {
		use super::*;

		#[test]
		fn valid_signals() {
			// Simple
			assert_eq!(Signal::request_connection().validate(), Ok(()));
			assert_eq!(Signal::reject_connection().validate(), Ok(()));
			assert_eq!(Signal::accept_connection().validate(), Ok(()));
			assert_eq!(Signal::connected().validate(), Ok(()));

			// 2-element compound
			assert_eq!(Signal::request_connection().with_message().validate(), Ok(()));
			assert_eq!(Signal::reject_connection().with_message().validate(), Ok(()));
			assert_eq!(Signal::accept_connection().indexed().validate(), Ok(()));
			assert_eq!(Signal::accept_connection().with_ack_mask().validate(), Ok(()));
			assert_eq!(Signal::accept_connection().with_message().validate(), Ok(()));
			assert_eq!(Signal::connected().indexed().validate(), Ok(()));
			assert_eq!(Signal::connected().with_ack_mask().validate(), Ok(()));
			assert_eq!(Signal::connected().with_message().validate(), Ok(()));

			// 3-element compound
			assert_eq!(Signal::accept_connection().indexed().with_ack_mask().validate(), Ok(()));
			assert_eq!(Signal::accept_connection().indexed().with_message().validate(), Ok(()));
			assert_eq!(Signal::accept_connection().indexed().with_stream().validate(), Ok(()));
			assert_eq!(Signal::accept_connection().with_ack_mask().with_message().validate(), Ok(()));
			assert_eq!(Signal::connected().indexed().with_ack_mask().validate(), Ok(()));
			assert_eq!(Signal::connected().indexed().with_message().validate(), Ok(()));
			assert_eq!(Signal::connected().indexed().with_stream().validate(), Ok(()));
			assert_eq!(Signal::connected().with_ack_mask().with_message().validate(), Ok(()));

			// 4-element compound
			assert_eq!(
				Signal::accept_connection().indexed().with_ack_mask().with_message()
					.validate(),
				Ok(())
			);
			assert_eq!(
				Signal::accept_connection().indexed().with_ack_mask().with_stream()
					.validate(),
				Ok(())
			);
			assert_eq!(
				Signal::accept_connection().indexed().with_message().with_stream()
					.validate(),
				Ok(())
			);
			assert_eq!(
				Signal::connected().indexed().with_ack_mask().with_message()
					.validate(),
				Ok(())
			);
			assert_eq!(
				Signal::connected().indexed().with_ack_mask().with_stream()
					.validate(),
				Ok(())
			);
			assert_eq!(
				Signal::connected().indexed().with_message().with_stream()
					.validate(),
				Ok(())
			);

			// 5-element compound
			assert_eq!(
				Signal::accept_connection().indexed().with_ack_mask().with_message().with_stream()
					.validate(),
				Ok(())
			);
			assert_eq!(
				Signal::connected().indexed().with_ack_mask().with_message().with_stream()
					.validate(),
				Ok(())
			);
		}
	}
}

pub mod index {
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
}

pub mod ack {
	use super::index::ParcelIndex;

	/// Mask that acknowledges received parcels.
	#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
	pub struct AckMask {
		last_index: ParcelIndex,
		mask: u64,
	}

	/// Acknowledging provided parcel index would result in skipping (missing) a parcel.
	#[derive(Debug)]
	pub struct AckError;

	impl AckMask {
		/// Construct a new **AckMask** that only acknowledges provided parcel index.
		pub fn new(acknowledged_parcel: ParcelIndex) -> Self {
			Self {
				last_index: acknowledged_parcel,
				mask: 0,
			}
		}

		/// Check whether the mask acknowledges provided parcel index.
		#[inline]
		pub fn acknowledges(&self, index: ParcelIndex) -> bool {
			let dist = ParcelIndex::dist(self.last_index, index);
			match dist {
				0 => true,
				x if x <= 64 => {
					let mask = 1 << (x - 1);
					self.mask & mask == mask
				},
				_ => false,
			}
		}

		/// Acknowledge provided parcel index without checking bounds.
		///
		/// # Note
		/// Using this function directly may cause reliable parcels to be skipped, breaking GNet
		/// guarantees. Prefer using [`ack`](Self::ack) instead.
		pub fn unchecked_ack(&mut self, index: ParcelIndex) {
			let dist = ParcelIndex::dist(self.last_index, index);
			match dist {
				x if x <= 64 => {
					let mask = 1 << (dist - 1);
					self.mask |= mask;
				},
				x if x >= 128 => {
					self.last_index = index;
					let d = u8::MAX - x;
					self.mask <<= d + 1;
					self.mask |= 1 << d;
				},
				_ => (),
			}
		}

		/// Acknowledge provided parcel index.
		///
		/// # Note
		/// Safer version of [`unchecked_ack`](Self::unchecked_ack).
		///
		/// # Returns
		/// Error if acknowledging provided parcel index would cause an unacknowledged previous
		/// index to go out of range, which may result in missed reliable parcels.
		pub fn ack(&mut self, index: ParcelIndex) -> Result<(), AckError> {
			let dist = ParcelIndex::dist(self.last_index, index);
			match dist {
				x if x <= 64 => {
					let mask = 1 << (dist - 1);
					self.mask |= mask;
					Ok(())
				},
				x if x <= 127 => Ok(()),
				x => {
					let d = u8::MAX - x;
					if self.mask.leading_ones() >= d as u32 {
						self.last_index = index;
						self.mask <<= d + 1;
						self.mask |= 1 << d;
						Ok(())
					} else {
						Err(AckError)
					}
				}
			}
		}

		/// Return little-endian serialization of Self.
		#[inline]
		pub fn to_le_bytes(&self) -> [u8; 9] {
			let mut bytes = [0; 9];
			bytes.copy_from_slice(&self.mask.to_le_bytes());
			bytes[8] = self.last_index.into();
			bytes
		}

		/// Deserialize Self from little-endian serialization.
		#[inline]
		pub fn from_le_bytes(bytes: [u8; 9]) -> Self {
			let mut mask_bytes = [0; 8];
			mask_bytes.copy_from_slice(&bytes[..8]);
			Self {
				last_index: bytes[8].into(),
				mask: u64::from_le_bytes(mask_bytes),
			}
		}

		/// Return big-endian serialization of Self.
		#[inline]
		pub fn to_be_bytes(&self) -> [u8; 9] {
			let mut bytes = [0; 9];
			bytes[0] = self.last_index.into();
			bytes[1..].copy_from_slice(&self.mask.to_be_bytes());
			bytes
		}

		/// Deserialize Self from big-endian serialization.
		#[inline]
		pub fn from_be_bytes(bytes: [u8; 9]) -> Self {
			let mut mask_bytes = [0; 8];
			mask_bytes.copy_from_slice(&bytes[1..]);
			Self {
				last_index: bytes[0].into(),
				mask: u64::from_be_bytes(mask_bytes),
			}
		}
	}

	#[cfg(test)]
	mod test {
		use super::*;

		#[test]
		fn ack_mask_acknowledges_initial() {
			let ack_mask = AckMask::new(12.into());
			ack_mask.acknowledges(12.into());
		}

		#[test]
		fn ack_mask_acknowledges_next() {
			let mut ack_mask = AckMask::new(12.into());
			ack_mask.ack(13.into()).unwrap();
			ack_mask.acknowledges(12.into());
			assert!(ack_mask.acknowledges(13.into()))
		}

		#[test]
		fn ack_mask_acknowledges_prev() {
			let mut ack_mask = AckMask::new(12.into());
			ack_mask.ack(11.into()).unwrap();
			ack_mask.acknowledges(12.into());
			assert!(ack_mask.acknowledges(11.into()))
		}

		#[test]
		fn ack_mask_acknowledges_sequential() {
			let mut ack_mask = AckMask::new(0.into());
			for i in 1..=u8::MAX {
				ack_mask.ack(i.into()).unwrap();
				assert!(ack_mask.acknowledges(i.into()));
				assert!(ack_mask.acknowledges((i - 1).into()));
			}
		}

		#[test]
		fn ack_mask_error_on_large_jump() {
			let mut ack_mask = AckMask::new(12.into());
			ack_mask.ack(82.into())
				.expect_err("Acknowledging more 70 indices ahead did not raise error.");
		}
	}
}

mod header {
	use super::{HandshakeId, signal::Signal, index::ParcelIndex, ack::AckMask};

	use crate::connection::id::ConnectionId;

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
			if self.signal.is_connected() {
				Some(self.connection_id)
			} else {
				None
			}
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

		/// Get the number of bytes the header takes up in a parcel.
		#[inline]
		pub fn size(&self) -> usize {
			signalled_size(self.signal)
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
}

type HandshakeId = u32;

pub use header::Header;
