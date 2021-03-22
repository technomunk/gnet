//! Test the GNet ability to connect a single client to a single server.
//!
//! Makes sure both endpoints are aware that the connection is established.

use std::net::{SocketAddr, UdpSocket};
use gnet::connection::parcel::{Header};

#[test]
fn single_client_test() {
	const REQUEST_MESSAGE: &str = "Please?";

	// Initial client data
	let client_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 2100));
	let client_socket = UdpSocket::bind(client_addr).expect("Failed to bind client socket");
	let mut client_buffer = [0; 1200];

	// Initial server data
	let server_addr = SocketAddr::from(([ 127, 0, 0, 1, ], 2101));
	let server_socket = UdpSocket::bind(server_addr).expect("Failed to bind server socket");
	let mut server_buffer = [0; 1200];

	// Build request parcel
	{
		let message = REQUEST_MESSAGE.as_bytes();
		let header = Header::request_connection(42).with_message(message.len() as u16);
		header.write_to(&mut client_buffer);
		header.mut_message_slice(&mut client_buffer).unwrap().copy_from_slice(message);
	}

	// Send request
	{
		let sent_byte_count = client_socket.send_to(&client_buffer, server_addr)
			.expect("Failed to send request packet");
		assert_eq!(sent_byte_count, client_buffer.len());
	}

	// Receive request
	{
		let (received_byte_count, sender_addr) = server_socket.recv_from(&mut server_buffer)
			.expect("Failed to receive request packet");
		assert_eq!(received_byte_count, client_buffer.len());
		assert_eq!(sender_addr, client_addr);
	}

	// Process and validate received parcel
	{
		let (header, _) = Header::read_from(&server_buffer).expect("Failed to deserialize header");
		assert!(header.signal().is_connection_request());
		let received_message = header.message_slice(&server_buffer).unwrap();
		assert_eq!(received_message, REQUEST_MESSAGE.as_bytes());
	}
	
	// TODO: accept request - ConnectionAcceptor
	// TODO: construct server-side connection

	// Build accept parcel
	{
		let header = Header::accept_connection(42, 1);
		header.write_to(&mut server_buffer);
	}

	// Send accept
	{
		let sent_byte_count = server_socket.send_to(&server_buffer, client_addr)
			.expect("Failed to send accept packet");
		assert_eq!(sent_byte_count, server_buffer.len());
	}

	// Receive accept
	{
		let (received_byte_count, sender_addr) = client_socket.recv_from(&mut client_buffer)
			.expect("Failed to receive accept packet");
		assert_eq!(received_byte_count, server_buffer.len());
		assert_eq!(sender_addr, server_addr);
	}

	// TODO: construct client-side connection
	// TODO: assert both client and server are aware of their connected state
}
