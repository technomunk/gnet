//! Mechanism for acknowledging received parcels.

use super::parcel::ParcelIndex;

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
