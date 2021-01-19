use std::sync::Arc;
use std::net::SocketAddr;
use gnet::{Connection, ConnectionListener, Parcel};
use gnet::byte::ByteSerialize;
use gnet::endpoint::Open;
use gnet::endpoint::basic::{ClientEndpoint, ServerEndpoint};

#[derive(Debug, PartialEq, Clone)]
enum TestParcel {
	String(String),
	Index(u64),
}

impl ByteSerialize for TestParcel {
	fn byte_count(&self) -> usize {
		match self {
			Self::String(string) => string.len() + 3,
			Self::Index(value) => 1 + value.byte_count(),
		}
	}

	fn to_bytes(&self, bytes: &mut [u8]) {
		match self {
			Self::String(string) => {
				0u8.to_bytes(bytes);
				let byte_count = string.len() as u16;
				byte_count.to_bytes(&mut bytes[1 ..]);
				let bytes = &mut bytes[byte_count.byte_count() + 1 ..];
				bytes.copy_from_slice(string.as_bytes());
			},
			Self::Index(value) => {
				1u8.to_bytes(bytes);
				value.to_bytes(&mut bytes[1 ..]);
			}
		}
	}

	fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), gnet::byte::SerializationError> {
		let (variant_index, offset) = u8::from_bytes(bytes)?;
		match variant_index {
			0 => {
				let (byte_count, extra_offset) = u16::from_bytes(&bytes[offset ..])?;
				let string = String::from_utf8(bytes[offset .. offset + byte_count as usize].to_vec())?;
				Ok((Self::String(string), offset + extra_offset + byte_count as usize))
			},
			1 => {
				let (value, extra_offset) = u64::from_bytes(&bytes[offset ..])?;
				Ok((Self::Index(value), offset + extra_offset))
			},
			_ => Err(gnet::byte::SerializationError::UnexpectedValue),
		}
	}
}

impl Parcel for TestParcel {}

#[test]
fn single_client_test() {
	const REQUEST_PAYLOAD: &[u8] = b"Single Client Test Connection Request";

	let test_parcel = TestParcel::String("Hello there friend!".to_string());

	let listener_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 2100));
	let client_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 2101));

	let listener = ConnectionListener::new(
		Arc::new(
			ServerEndpoint::open(listener_addr)
				.expect("Failed to open server socket!")));
	let client_connection = Connection::connect(
		ClientEndpoint::open(client_addr).expect("Failed to open client socket!"),
		listener_addr,
		REQUEST_PAYLOAD.to_vec(),
	).expect("Failed to begin establishing a connection from the client!");

	let accept_result = listener.try_accept(|addr, payload| {
		if addr == client_addr && payload == REQUEST_PAYLOAD {
			gnet::listener::AcceptDecision::Allow
		} else {
			gnet::listener::AcceptDecision::Ignore
		}
	});

	let mut server_connection = accept_result.expect("Failed to accept client connection!");
	server_connection.push_reliable_parcel(test_parcel.clone()).unwrap();
	server_connection.flush().unwrap();

	let mut client_connection = client_connection.try_promote().unwrap();
	let (received_parcel, _) = client_connection.pop_parcel().unwrap();

	assert_eq!(test_parcel, received_parcel);
}
