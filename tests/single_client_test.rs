use gnet::byte::ByteSerialize;
use gnet::connection::context::Context;
use gnet::connection::parcel;
use std::net::{SocketAddr, UdpSocket};

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

impl gnet::connection::Parcel for TestParcel {}

#[test]
fn single_client_test() {
	const REQUEST_PAYLOAD: &[u8] = b"Single Client Test Connection Request";

	let mut byte_buffer = vec![0; 1200].into_boxed_slice();

	let test_parcel = TestParcel::String("Hello there friend!".to_string());

	let listener_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 2100));
	let client_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 2101));

	// Set up listener
	// TODO:
	let listener_socket = UdpSocket::bind(listener_addr).expect("Faild to bind listener socket.");
	// Listener::new()
	
	// Set up client
	let client_socket = UdpSocket::bind(client_addr).expect("Failed to bind client socket");
	let mut client_context = Context::<TestParcel>::pending();

	// Connect
	let len = client_context.build_request_packet(&mut byte_buffer, REQUEST_PAYLOAD).unwrap();
	client_socket.send_to(&byte_buffer[.. len], listener_addr).unwrap();
	
	// Accept
	let (recv_bytes, recv_addr) = listener_socket.recv_from(&mut byte_buffer).unwrap();
	assert_eq!(recv_addr, client_addr);
	// TODO: listener utility that allows constructing accept packet
	// assert_eq!(packet::get_data_segment(&byte_buffer[.. recv_bytes]), REQUEST_PAYLOAD);

	// TODO: send and receive parcels from both ends
}
