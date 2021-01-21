//! Generic functions for testing [`Demux`](Demux) implementations.

use super::Demux;

use std::collections::HashMap;
use std::net::SocketAddr;

/// Test that provided [`Demux`](Demux) implementation behaves as expected.
pub fn generic_demux_test<D: Demux<u32>>(demultiplexer: &mut D) {
	let datagrams: [(&[u8], SocketAddr); 3] = [
		(b"0", SocketAddr::from(([ 127, 0, 0, 1, ], 0))),
		(b"1-0", SocketAddr::from(([ 127, 0, 0, 1, ], 1))),
		(b"1-2", SocketAddr::from(([ 127, 0, 0, 1, ], 2))),
	];

	assert!(!demultiplexer.is_allowed(0));
	assert!(!demultiplexer.is_allowed(1));

	demultiplexer.allow(0);
	assert!(demultiplexer.is_allowed(0));
	assert!(!demultiplexer.is_allowed(1));

	demultiplexer.allow(1);
	assert!(demultiplexer.is_allowed(0));
	assert!(demultiplexer.is_allowed(1));

	demultiplexer.push(0, datagrams[0]);
	demultiplexer.push(1, datagrams[1]);
	demultiplexer.push(1, datagrams[2]);
	demultiplexer.block(0);
	demultiplexer.allow(0);

	demultiplexer.process(0, |_| panic!("Blocked datagrams were not dropped!"));

	let mut found_dgrams = [false; 3];
	demultiplexer.process(1, |dgram| {
		let index = datagrams
			.iter()
			.position(|item| *item == dgram)
			.expect("Failed to find a sent datagram!");
		assert!(!found_dgrams[index]);
		found_dgrams[index] = true;
	});
	assert!(!found_dgrams[0], "Processed datagram not associated with required key!");
	assert!(found_dgrams[1], "Did not process a buffered datagram!");
	assert!(found_dgrams[2], "Did not process a buffered datagram!");

	demultiplexer.process(1, |_| panic!("Did not unbuffered processed datagrams!"));
}

#[test]
fn hash_map_demultiplexes() {
	let mut hash_map = HashMap::new();
	generic_demux_test(&mut hash_map);
}
