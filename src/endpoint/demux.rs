//! [`Demux`](Demux) trait definition, implementation and test.

// #[cfg(feature = "basic-endpoints")]
// pub mod basic;
#[cfg(test)]
pub mod test;

pub use crate::id::ConnectionId;

use std::collections::HashMap;
use std::hash::Hash;
use std::net::SocketAddr;

/// A trait for connection demultiplexers.
///
/// Demultiplexers allow multiple connections to use the same endpoint simultaneously by
/// inspecting arriving datagrams and buffering any that belong to allowed connections for
/// later use.
pub trait Demux<K> {
	/// Allow buffering datagrams associated with provided key.
	fn allow(&mut self, key: K);
	/// Block (disallow) buffering datagrams associated with provided key.
	///
	/// # Note
	/// Any datagrams already associated with the newly blocked key should be dropped.
	fn block(&mut self, key: K);
	/// Check whether buffering datagrams associated with provided key is currently allowed.
	fn is_allowed(&self, key: K) -> bool;

	/// Buffer a datagram associated with provided key.
	///
	/// # Notes
	/// - The length and source address of the datagram should be recorded as it needs
	/// to be returned with [`pop`](Mux::pop).
	/// - The connection may be assumed to be allowed at the time of invocation.
	/// - The implementation may assume the key is allowed at the time of invocation.
	fn push(&mut self, key: K, dgram: (&[u8], SocketAddr));

	/// Process buffered datagrams associated with provided key by invoking the provided functor.
	///
	/// # Notes
	/// - The functor should be invoked exactly once for each buffered datagram.
	/// - The order of invocations is up to the implementation.
	/// - The implementation may assume the key is allowed at the time of invocation.
	fn process<F: FnMut((&[u8], SocketAddr))>(&mut self, key: K, functor: F);
}

impl<K: Hash + Eq> Demux<K> for HashMap<K, (Vec<u8>, Vec<(usize, SocketAddr)>)> {
	#[inline]
	fn allow(&mut self, key: K) {
		self.entry(key).or_insert_with(Default::default);
	}
	#[inline]
	fn block(&mut self, key: K) {
		self.remove(&key);
	}
	#[inline]
	fn is_allowed(&self, key: K) -> bool {
		self.contains_key(&key)
	}

	fn push(&mut self, key: K, dgram: (&[u8], SocketAddr)) {
		let (bytes, infos) = self.get_mut(&key).unwrap();
		bytes.extend_from_slice(dgram.0);
		infos.push((dgram.0.len(), dgram.1));
	}
	fn process<F: FnMut((&[u8], SocketAddr))>(&mut self, key: K, mut functor: F) {
		let (bytes, infos) = self.get_mut(&key).unwrap();
		let mut offset = 0;
		for (len, src) in infos.iter() {
			functor((&bytes[offset .. offset + *len], *src));
			offset += *len;
		}
		infos.clear();
		bytes.clear();
	}
}
