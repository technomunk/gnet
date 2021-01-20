//! Generic functions for testing [`Demux`](Demux) implementations.

use super::Demux;

use std::cmp::max;
use std::net::SocketAddr;

const DATAGRAMS: [(&[u8], SocketAddr); 3] = [
	(b"0", SocketAddr::from(([ 127, 0, 0, 1, ], 0))),
	(b"1-0", SocketAddr::from(([ 127, 0, 0, 1, ], 1))),
	(b"1-2", SocketAddr::from(([ 127, 0, 0, 1, ], 2))),
];

/// Test that provided [`Demux`](Demux) implementation behaves as expected.
pub fn generic_demux_test<D: Demux>(demultiplexer: &mut D) {
	assert!(!demultiplexer.is_allowed(0));
	assert!(!demultiplexer.is_allowed(1));

	demultiplexer.allow(0);
	assert!(demultiplexer.is_allowed(0));
	assert!(!demultiplexer.is_allowed(1));

	demultiplexer.allow(1);
	assert!(demultiplexer.is_allowed(0));
	assert!(demultiplexer.is_allowed(1));

	demultiplexer.push(0, DATAGRAMS[0].1, DATAGRAMS[0].0);
	demultiplexer.push(1, DATAGRAMS[1].1, DATAGRAMS[0].0);
	demultiplexer.block(0);

	let mut buffer = Vec::with_capacity(DATAGRAMS.iter().fold(0, |acc, x| max(acc, x.0.len())));

	assert_eq!(demultiplexer.pop(0, &mut buffer), None);

	// Since the pop order is relaxed either one is correct
	let popped_info = demultiplexer.pop(1, &mut buffer).unwrap();
	if &buffer[.. DATAGRAMS[1].0.len()] == DATAGRAMS[1].0 {
		assert_eq!(popped_info, (DATAGRAMS[1].0.len(), DATAGRAMS[1].1));
		assert_eq!(demultiplexer.pop(1, &mut buffer), Some((DATAGRAMS[2].0.len(), DATAGRAMS[2].1)));
		assert_eq!(&buffer[.. DATAGRAMS[2].0.len()], DATAGRAMS[2].0);
	} else {
		assert_eq!(&buffer[.. DATAGRAMS[2].0.len()], DATAGRAMS[2].0);
		assert_eq!(popped_info, (DATAGRAMS[2].0.len(), DATAGRAMS[2].1));
		assert_eq!(demultiplexer.pop(1, &mut buffer), Some((DATAGRAMS[1].0.len(), DATAGRAMS[1].1)));
		assert_eq!(&buffer[.. DATAGRAMS[1].0.len()], DATAGRAMS[1].0)
	}

	assert_eq!(demultiplexer.pop(1, &mut buffer), None);
}
