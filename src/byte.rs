//! Definition of byte serialization trait and helper structs.

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

/// An error occurring during byte-serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum SerializationError {
	/// Serialization would cause a buffer-overflow.
	BufferOverflow,
	/// Encountered an unexpected (uninterpretable) value during serialization.
	UnexpectedValue,
}

// TODO: custom #[derive(ByteSerialize)]
/// A trait for objects that can be written to or read from a byte-stream.
///
/// Correct implementations of this trait fulfil following predicates:
/// - A call to [`to_bytes`](Self::to_bytes) must write no more than
/// [`byte_count`](Self::byte_count) bytes.
/// - The byte-stream produced by a call to [`to_bytes`](Self::to_bytes) should produce a valid
/// object on call of [`from_bytes`](Self::from_bytes).
///
/// `ByteSerialize` is implemented by default for:
/// - Empty type. (`()`)
/// - Trivial types. (ex: `u8`, `usize`, `float`).
/// - Arrays of `ByteSerialize + Default` objects up to size 32.
/// (ex: `[f32; 3]`, `[[f32; 4]; 4]`, `[u8; 4]`).
/// - Tuples of `ByteSerialize` objects.
/// (ex: `(f32, f64, u16)`, `([u16; 4], u16)`, `((i32, isize), usize)`).
pub trait ByteSerialize: Sized {
	/// Size of the serialization of the object in bytes.
	fn byte_count(&self) -> usize;

	/// Serialize self to a byte-stream.
	///
	/// The stream is guaranteed to be at least [`self.byte_count()`](Self::byte_count) large.
	/// Exactly [`self.byte_count()`](Self::byte_count) bytes should be written!
	fn to_bytes(&self, bytes: &mut [u8]);

	/// Construct Self from a byte-stream.
	///
	/// Should produce a constructed instance of [`Self`](Self) and the number of bytes read.
	/// The number of bytes read should be exactly equal to [`byte_count()`](Self::byte_count)
	/// of the returned object!
	fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), SerializationError>;
}

mod standard;

impl Display for SerializationError {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		write!(f, "serialization would cause buffer overflow")
	}
}

impl Error for SerializationError {}

impl From<std::string::FromUtf8Error> for SerializationError {
	#[inline]
	fn from(_: std::string::FromUtf8Error) -> Self {
		Self::UnexpectedValue
	}
}
