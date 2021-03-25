//! Utilities for providing reliability to delivered parcels.

use super::parcel::ParcelIndex;
use super::ack::AckMask;

/// Manager responsible for making sure the user-app data is delivered reliably.
#[derive(Clone, Debug, Default)]
pub struct DeliveryManager {
	next_index: ParcelIndex,
	ack_mask: Option<AckMask>,
}

impl DeliveryManager {
	/// Get the next free [`ParcelIndex`](ParcelIndex).
	///
	/// # Returns
	/// The first unused parcel index if it can be acknowledged by the other end or `None` if
	/// the other end of the connection will not be able to acknowledge all pending parcels.
	pub fn next_index(&mut self) -> Option<ParcelIndex> {
		if let Some(mask) = &self.ack_mask {
			mask.acknowledges(self.next_index - 64)
				.then(|| {
					let index = self.next_index;
					self.next_index = self.next_index.next();
					index
				})
		} else if self.next_index <= ParcelIndex::from(65) {
			let index = self.next_index;
			self.next_index = self.next_index.next();
			Some(index)
		} else {
			None
		}
	}

	/// Acknowledge delivered parcels according to provided [`AckMask`](AckMask).
	///
	/// # Note
	/// The provided mask should come from the other end of the connection, ie signal parcels
	/// received by it.
	pub fn acknowledge(&mut self, ack_mask: &AckMask) {
		
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn delivery_manager_runs_out_of_indices() {
		let mut manager = DeliveryManager::default();
		for i in 0..=65 {
			assert_eq!(manager.next_index(), Some(ParcelIndex::from(i)));
		}
		assert_eq!(manager.next_index(), None);
	}
}
