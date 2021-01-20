//! Connection Id management.

use std::num::Wrapping;

/// A unique index associated with a connection.
///
/// **NOTE**: `0` is a special value that means `no-connection-id`.
pub type ConnectionId = u16;

/// All possible connection ids have been used up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutOfIdsError();

/// Manager for [`ConnectionIds`](ConnectionId). Responsible for making sure
/// there are no [`Connections`](super::connection::Connection) that share ids.
#[derive(Debug)]
pub struct Allocator {
	/// Largest ConnectionId in use.
	last_id: ConnectionId,
	/// Collection of free ids that may be used.
	free_ids: Vec<ConnectionId>,
}

impl Allocator {
	/// Assign a new [`ConnectionId`](ConnectionId).
	pub fn allocate(&mut self) -> Result<ConnectionId, OutOfIdsError> {
		if self.free_ids.is_empty() {
			if self.last_id == ConnectionId::MAX {
				Err(OutOfIdsError())
			} else {
				self.last_id += 1;
				Ok(self.last_id)
			}
		} else {
			Ok(self.free_ids.pop().unwrap())
		}
	}

	/// Mark provided [`ConnectionId`](ConnectionId) as free to use.
	/// 
	/// Has `O(N)` complexity, where N is the number of elements in `self.free_ids` vector.
	pub fn free(&mut self, id: ConnectionId) {
		if id == self.last_id {
			self.last_id -= 1;
			while ! self.free_ids.is_empty() && *self.free_ids.last().unwrap() == self.last_id {
				self.free_ids.pop();
				self.last_id -= 1
			}
		} else if let Some(pos) = self.free_ids.iter().position(|&x| x > id) {
			self.free_ids.insert(pos, id)
		} else {
			self.free_ids.push(id)
		}
	}
}

impl Default for Allocator {
	fn default() -> Self {
		Self {
			last_id: 0,
			free_ids: Vec::new(),
		}
	}
}

impl std::fmt::Display for OutOfIdsError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
		write!(f, "Connection Id Allocator ran out of ids!")
	}
}

impl std::error::Error for OutOfIdsError {}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn allocator_reuses_ids() {
		let mut allocator = Allocator::default();
		let ids = [
			allocator.allocate().unwrap(),
			allocator.allocate().unwrap(),
			allocator.allocate().unwrap(),
		];

		allocator.free(ids[0]);
		allocator.free(ids[1]);

		assert_eq!(allocator.allocate().unwrap(), ids[1]);
		assert_eq!(allocator.allocate().unwrap(), ids[0])
	}

	#[test]
	fn allocator_reuses_all_ids() {
		let mut allocator = Allocator::default();
		let ids = [
			allocator.allocate().unwrap(),
			allocator.allocate().unwrap(),
			allocator.allocate().unwrap(),
			allocator.allocate().unwrap(),
		];

		allocator.free(ids[2]);
		allocator.free(ids[0]);
		allocator.free(ids[1]);
		allocator.free(ids[3]);

		assert!(allocator.free_ids.is_empty());
		assert_eq!(allocator.last_id, 0)
	}

	#[test]
	fn allocator_runs_out_of_ids_before_0() {
		let mut allocator = Allocator::default();
		for id in 0 .. ConnectionId::MAX {
			assert_eq!(allocator.allocate().unwrap(), id + 1);
		};

		assert!(allocator.allocate().is_err())
	}
}
