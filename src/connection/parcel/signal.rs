//! Signalling bitmask that is used by parcel [`Headers`](super::Header).

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
