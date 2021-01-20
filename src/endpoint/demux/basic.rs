//! Basic demultiplexer implementation.

use super::{ConnectionId, Demux};

use std::collections::HashMap;
use std::net::SocketAddr;

type DatagramInfo = (usize, SocketAddr);

/// Basic [`Demux`](Demux) trait implementation.
///
/// The implementation contains memory allocation reuse optimization, but does not make any
/// assumptions about its usage. See the [trait documentation](Demux) for more details.
#[derive(Debug, Default)]
pub struct Demultiplexer {
	connections: HashMap<ConnectionId, (Vec<u8>, Vec<DatagramInfo>)>,
	free_vectors: Vec<(Vec<u8>, Vec<DatagramInfo>)>,
}

impl Demultiplexer {

	/// Construct a new demultiplexer.
	pub fn new() -> Self {
		Self {
			connections: HashMap::new(),
			free_vectors: Vec::new(),
		}
	}

	/// Construct a new demultiplexer and preallocate resources for provided number of connections.
	pub fn with_capacity(capacity: usize) -> Self {
		let mut free_vectors = Vec::with_capacity(capacity);
		free_vectors.resize(capacity, (Vec::new(), Vec::new()));
		Self {
			connections: HashMap::with_capacity(capacity),
			free_vectors,
		}
	}
}

impl Demux for Demultiplexer {
	fn allow(&mut self, connection: ConnectionId) {
		if !self.connections.contains_key(&connection) {
			self.connections.insert(connection, self.free_vectors.pop().unwrap_or_default());
		}
	}
	fn block(&mut self, connection: ConnectionId) {
		if let Some(vectors) = self.connections.remove(&connection) {
			self.free_vectors.push(vectors)
		}
	}
	#[inline]
	fn is_allowed(&self, connection: ConnectionId) -> bool {
		self.connections.contains_key(&connection)
	}

	fn push(&mut self, connection: ConnectionId, addr: SocketAddr, datagram: &[u8]) {
		let (bytes, infos) = self.connections.get_mut(&connection).unwrap();
		bytes.extend_from_slice(datagram);
		infos.push((datagram.len(), addr));
	}
	fn pop(&mut self, connection: ConnectionId, buffer: &mut [u8]) -> Option<(usize, SocketAddr)> {
		let (bytes, infos) = self.connections.get_mut(&connection).unwrap();
		let datagram_info = infos.pop()?;
		buffer.copy_from_slice(&bytes[bytes.len() - datagram_info.0 ..]);
		Some(datagram_info)
	}
}

#[cfg(test)]
#[test]
fn demultiplexer_works() {
	let mut demultiplexer = Demultiplexer::new();
	super::test::generic_demux_test(&mut demultiplexer)
}
