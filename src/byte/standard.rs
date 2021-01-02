//! Implementations of [`ByteSerialize`](super::ByteSerialize) for standard library types.

use super::{ByteSerialize, SerializationError};

use std::mem::size_of;

macro_rules! impl_byte_serialize_numeric {
	() => {};
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
		if bytes.is_empty() {
			Err(SerializationError::BufferOverflow)
		} else {
			Ok((bytes[0] != 0, 1))
		}
	}
}

macro_rules! impl_byte_serialize_generic_array {
	() => {};
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

impl_byte_serialize_generic_array!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32);

macro_rules! impl_byte_serialize_tuple {
	() => {};
	($(($name:ident, $element:ident, $index:tt),)+) => {
		impl<$($name: ByteSerialize),+> ByteSerialize for ($($name,)+) {
			#[inline]
			fn byte_count(&self) -> usize {
				let mut result = 0;
				$(
					result += self.$index.byte_count();
				)+
				result
			}

			#[inline]
			#[allow(unused_assignments)]
			fn to_bytes(&self, bytes: &mut [u8]) {
				let mut offset = 0;
				// cache sizes of elements
				$(let $element = self.$index.byte_count();)+
				// calculate total size
				$(offset += $element;)+
				// read individual elements
				$(
					offset -= $element;
					self.$index.to_bytes(&mut bytes[offset..]);
				)+
			}

			#[inline]
			fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), SerializationError> {
				let mut total_processed_bytes = 0;
				$(
					let ($element, processed_bytes) = $name::from_bytes(&bytes[total_processed_bytes..])?;
					total_processed_bytes += processed_bytes;
				)+
				Ok((($($element,)+), total_processed_bytes))
			}
		}

		peel_impl_byte_serialize_tuple!{$(($name, $element, $index),)+}
	};
}

macro_rules! peel_impl_byte_serialize_tuple {
	($first:expr, $(($name:ident, $element:ident, $index:tt),)*) => { impl_byte_serialize_tuple!{$(($name, $element, $index),)*} }
}

impl_byte_serialize_tuple!{ (T11, e11, 11), (T10, e10, 10), (T9, e9, 9), (T8, e8, 8), (T7, e7, 7), (T6, e6, 6), (T5, e5, 5), (T4, e4, 4), (T3, e3, 3), (T2, e2, 2), (T1, e1, 1), (T0, e0, 0),}

#[cfg(test)]
mod test {
	use super::ByteSerialize;

	#[test]
	fn u32_serializes() {
		let original: u32 = 0xDEAD_BEEF;
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
	fn single_element_tuple_serializes() {
		let original: (u32,) = (0xDEAD_BEEF,);
		let mut bytes = [0xFF; 4];

		assert_eq!(original.byte_count(), 4);

		original.to_bytes(&mut bytes);

		assert_eq!(bytes, [0xEF, 0xBE, 0xAD, 0xDE]);

		let (deserialized, byte_count) = <(u32,)>::from_bytes(&bytes).unwrap();

		assert_eq!(byte_count, 4);
		assert_eq!(original, deserialized);
	}

	#[test]
	fn two_element_tuple_serializes() {
		let original: (u32, f32) = (0xDEAD_BEEF, std::f64::consts::PI as f32);
		let mut bytes = [0xFF; 8];

		assert_eq!(original.byte_count(), 8);
		
		original.to_bytes(&mut bytes);

		let pi_bytes = (std::f64::consts::PI as f32).to_le_bytes();
		assert_eq!(&bytes[..4], [0xEF, 0xBE, 0xAD, 0xDE]);
		assert_eq!(&bytes[4..], pi_bytes);

		let (deserialized, byte_count) = <(u32, f32)>::from_bytes(&bytes).unwrap();

		assert_eq!(byte_count, 8);
		assert_eq!(original, deserialized);
	}

	#[test]
	fn twelve_element_tuple_serializes() {
		type TestedType = (u8, i8, u16, i16, u32, i32, u64, i64, [u8; 1], [i8; 1], [u8; 2], [i8; 2]);
		const EXPECTED_BYTE_COUNT: usize = 36;

		let original: TestedType = (0, 1, 2, 3, 4, 5, 6, 7, [8], [9], [1, 0], [1, 1]);
		let mut bytes = [0xFF; EXPECTED_BYTE_COUNT];

		assert_eq!(original.byte_count(), EXPECTED_BYTE_COUNT);

		original.to_bytes(&mut bytes);
		let (deserialized, byte_count) = <TestedType>::from_bytes(&bytes).unwrap();

		assert_eq!(byte_count, EXPECTED_BYTE_COUNT);
		assert_eq!(original, deserialized);
	}
}
