//! Traits for representing types as individual bytes.

/// A given type can be serialized to and from a byte-stream.
/// 
/// `ByteSerialize` is implemented for numeric types, Vecs of `ByteSerialize` types and arrays of sizes 1 through 16 of `ByteSerialize` types.
pub trait ByteSerialize : Sized {
	/// Size of the serialized object in bytes.
	fn byte_count(&self) -> usize;

	/// Serialize self to a byte-stream.
	/// 
	/// The stream is guaranteed to be at least byte_count() large.
	fn to_bytes(&self, bytes: &mut [u8]);

	/// Construct Self from a byte-stream.
	fn from_bytes(bytes: &[u8]) -> (Self, usize);
}

mod standard;
