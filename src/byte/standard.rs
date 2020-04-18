//! Implementations for standard library types.

use super::{ByteSerialize, SerializationError};

use std::mem::size_of;

macro_rules! impl_byte_serialize_numeric {
	($type:ty) => {
		// NOTE: this implementation is highly specialized for trivial integer types, avoid using it as reference!
		// For a safe (and recommended) approach see implementation of `ByteSerialize` for `Vec<T>`.
		impl ByteSerialize for $type {
			#[inline]
			fn byte_count(&self) -> usize { size_of::<Self>() }
			#[inline]
			fn to_bytes(&self, bytes: &mut [u8]) {
				assert!(bytes.len() >= size_of::<Self>());
				unsafe { *(bytes.as_mut_ptr() as *mut _) = (*self).to_le_bytes(); };
			}
			#[inline]
			fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), SerializationError> {
				if bytes.len() < size_of::<Self>() {
					Err(SerializationError::BufferOverflow)
				} else {
					let result = Self::from_le_bytes(unsafe { *(bytes.as_ptr() as *const _) });
					Ok((result, size_of::<Self>()))
				}
			}
		}
	};
	($type:ty, $($another:ty),*) => (
		impl_byte_serialize_numeric!($type);
		impl_byte_serialize_numeric!($($another),*);
	);
}

impl_byte_serialize_numeric!(usize, isize);
impl_byte_serialize_numeric!(u8, i8);
impl_byte_serialize_numeric!(u16, i16);
impl_byte_serialize_numeric!(u32, i32, f32);
impl_byte_serialize_numeric!(u64, i64, f64);
impl_byte_serialize_numeric!(u128, i128);

impl ByteSerialize for bool {
	#[inline]
	fn byte_count(&self) -> usize { 1 }
	#[inline]
	fn to_bytes(&self, bytes: &mut [u8]) {
		bytes[0] = *self as u8;
	}
	#[inline]
	fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), SerializationError> {
		if bytes.len() < 1 {
			Err(SerializationError::BufferOverflow)
		} else {
			Ok((bytes[0] != 0, 1))
		}
	}
}

macro_rules! impl_byte_serialize_generic_array {
	($count:literal) => {
		impl<T: ByteSerialize + Default> ByteSerialize for [T; $count] {
			#[inline]
			fn byte_count(&self) -> usize {
				let mut byte_count = 0;
				for item in self {
					byte_count += item.byte_count();
				};
				byte_count
			}
			#[inline]
			fn to_bytes(&self, bytes: &mut [u8]) {
				assert!(bytes.len() >= self.byte_count());
				let mut processed_byte_count = 0;
				for item in self {
					item.to_bytes(&mut bytes[processed_byte_count..]);
					processed_byte_count += item.byte_count();
				}
			}
			#[inline]
			fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), SerializationError> {
				let mut result = Self::default();
				let mut processed_byte_count = 0;
				for i in 0..$count {
					let (item, item_bytes) = T::from_bytes(&bytes[processed_byte_count..])?;
					result[i] = item;
					processed_byte_count += item_bytes;
				};
				Ok((result, processed_byte_count))
			}
		}
	};
	($count:literal, $($another:literal),*) => (
		impl_byte_serialize_generic_array!($count);
		impl_byte_serialize_generic_array!($($another),*);
	);
}

// TODO/(RFC 1210): specialize collections of trivial types.

impl_byte_serialize_generic_array!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16);

impl<T: ByteSerialize> ByteSerialize for Vec<T> {
	fn byte_count(&self) -> usize {
		let mut byte_count = self.len().byte_count();
		for item in self {
			byte_count += item.byte_count();
		};
		byte_count
	}

	fn to_bytes(&self, bytes: &mut [u8]) {
		assert!(bytes.len() >= self.byte_count());
		self.len().to_bytes(bytes);
		let mut processed_byte_count = self.len().byte_count();
		for item in self {
			item.to_bytes(&mut bytes[processed_byte_count..]);
			processed_byte_count += item.byte_count();
		}
	}

	fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), SerializationError> {
		let (capacity, mut processed_byte_count) = usize::from_bytes(bytes)?;
		let mut result = Self::with_capacity(capacity);
		for _ in 0..capacity {
			let (item, item_byte_count) = T::from_bytes(&bytes[processed_byte_count..])?;
			result.push(item);
			processed_byte_count += item_byte_count;
		}
		Ok((result, processed_byte_count))
	}
}

#[cfg(test)]
mod test {
	use super::ByteSerialize;

	#[test]
	fn u32_serializes() {
		let original: u32 = 0xDEADBEEF;
		let mut bytes = [0; 4];

		assert_eq!(original.byte_count(), 4);

		original.to_bytes(&mut bytes);

		// We expect the serialized version to be little-endian.
		assert_eq!(bytes, [0xEF, 0xBE, 0xAD, 0xDE]);

		let (deserialized, byte_count) = u32::from_bytes(&bytes).unwrap();

		assert_eq!(byte_count, 4);
		assert_eq!(original, deserialized);
	}

	#[test]
	fn array_serializes() {
		let original = [0.1, 0.2, 0.5, 1e-6];
		let mut bytes = [0; 16];
		
		assert_eq!(original.byte_count(), 16);

		original.to_bytes(&mut bytes);
		let (deserialized, byte_count) = <[f32; 4]>::from_bytes(&bytes).unwrap();

		assert_eq!(byte_count, 16);
		assert_eq!(original, deserialized);
	}

	#[test]
	fn matrix_serializes() {
		let original = [
			[ 1.0, 0.0, 0.0, 0.3, ],
			[ 0.0, 2.0, 0.0, 0.2, ],
			[ 0.0, 0.0, 3.0, 0.1, ],
			[ 0.0, 0.0, 0.0, 1.0, ],
		];
		let mut bytes = [0; 64];

		assert_eq!(original.byte_count(), 64);

		original.to_bytes(&mut bytes);
		let (deserialized, byte_count) = <[[f32; 4]; 4]>::from_bytes(&bytes).unwrap();

		assert_eq!(byte_count, 64);
		assert_eq!(original, deserialized);
	}

	#[test]
	fn bool_array_serializes() {
		let original = [true, false, true];
		let mut bytes = [0xFF; 4];

		assert_eq!(original.byte_count(), 3);
		
		original.to_bytes(&mut bytes);

		assert_eq!(bytes, [0x01, 0x00, 0x01, 0xFF]);

		let (deserialized, byte_count) = <[bool; 3]>::from_bytes(&bytes).unwrap();

		assert_eq!(byte_count, 3);
		assert_eq!(original, deserialized);
	}

	#[test]
	fn vector_serializes() {
		let original = vec![1, 2, 3];
		let mut bytes = [0; 20];

		assert_eq!(original.byte_count(), 20);

		original.to_bytes(&mut bytes);
		let (deserialized, byte_count) = <Vec<i32>>::from_bytes(&bytes).unwrap();

		assert_eq!(byte_count, 20);
		assert_eq!(original, deserialized);
	}

	#[test]
	fn vector_of_vectors_serializes() {
		let original = vec![Vec::new(), vec![1, 2, 3], Vec::with_capacity(12)];
		let mut bytes = [0; 80];

		assert_eq!(original.byte_count(), 80);

		original.to_bytes(&mut bytes);
		let (deserialized, byte_count) = <Vec<Vec<i128>>>::from_bytes(&bytes).unwrap();

		assert_eq!(byte_count, 80);
		assert_eq!(original, deserialized);
	}
}
