//! Byte-serialization trait.

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

/// An error occurring during byte-serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum SerializationError {
	/// Serialization would cause a buffer-overflow.
	BufferOverflow,
}

// TODO: custom #[derive(ByteSerialize)]
/// A given type can be serialized to and from a byte-stream.
/// 
/// `ByteSerialize` is implemented by default for:
/// - Trivial types. (ex: `u8`, `usize`, `float`).
/// - Arrays of `ByteSerialize + Default` objects up to size 32. (ex: `[f32; 3]`, `[[f32; 4]; 4]`, `[u8; 4]`).
/// - Tuples of `ByteSerialize` objects. (ex: `(f32, f64, u16)`, `([u16; 4], u16)`, `((i32, isize), usize)`).
pub trait ByteSerialize : Sized {
	/// Size of the serialized object in bytes.
	fn byte_count(&self) -> usize;

	/// Serialize self to a byte-stream.
	/// 
	/// The stream is guaranteed to be at least byte_count() large.
	fn to_bytes(&self, bytes: &mut [u8]);

	/// Construct Self from a byte-stream.
	fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), SerializationError>;
}

mod standard;

impl Display for SerializationError {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		write!(f, "serialization would cause buffer overflow")
	}
}

impl Error for SerializationError {}
