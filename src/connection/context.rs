//! Connection context.

use super::Parcel;
use super::id::ConnectionId;
use super::error::{BuildPacketError, ConnectionError};

use std::marker::PhantomData;

/// State of a connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionStatus {
	/// Normal functioning state.
	///
	/// Connection's full functionality may be used.
	Open,

	/// Connection is being established.
	///
	/// It may not be used to transmit data yet.
	Pending,

	/// Connection has beed deemed lost, due to lack of received relevant network traffic. This
	/// may be caused by a sudden shutdown of the other end or due to network conditions.
	Lost,

	/// Connection has been explicitly closed by the other end.
	///
	/// Connection may only be dropped to free system resources.
	Closed,
}

/// Connection context.
///
/// Used for processing incoming and build outgoing datagrams.
pub struct Context<P: Parcel> {
	connection_id: ConnectionId,
	status: ConnectionStatus,
	buffer: Vec<u8>,

	_message_type: PhantomData<P>,
}

impl<P: Parcel> Context<P> {
	/// Construct a pending connection context.
	///
	/// A pending connection is not yet established and as such can not be used to transmit data
	/// between endpoints.
	pub fn pending() -> Self {
		Self {
			connection_id: 0,
			status: ConnectionStatus::Pending,
			buffer: Vec::new(),

			_message_type: Default::default(),
		}
	}

	/// Construct an accepted connection context with provided id.
	pub fn accept(connection_id: ConnectionId) -> Self {
		Self {
			connection_id,
			status: ConnectionStatus::Open,
			buffer: Vec::new(),
			
			_message_type: Default::default(),
		}
	}

	/// Get the current status (state) of the connection.
	#[inline]
	pub fn status(&self) -> ConnectionStatus {
		self.status
	}

	/// Get the connection id if the connection has one.
	///
	/// A [pending](ConnectionStatus::Pending) connection may not have a valid id yet.
	pub fn connection_id(&self) -> Option<ConnectionId> {
		if self.connection_id == 0 {
			None
		} else {
			Some(self.connection_id)
		}
	}

	/// Get the next processed parcel.
	///
	/// Includes the data prelude from the network packet that the parcel was transmitted with.
	pub fn pop_parcel(&mut self) -> Result<(P, [u8; 4]), ConnectionError> {
		todo!()
	}

	/// Queue provided parcel to be included in built packets.
	///
	/// Reliable parcels are guaranteed to be delivered as long as the connection
	/// is in a valid state. The order of delivery is not guaranteed however, for
	/// order-dependent functionality use streams.
	pub fn push_reliable_parcel(&mut self, parcel: P) -> Result<(), ConnectionError> {
		todo!()
	}

	/// Queue provided parcel to be included in built packets.
	///
	/// Unreliable (volatile) parcels are delivered in a best-effort manner, however no
	/// re-transmission occurs of the parcel was not received by the other end. The order
	/// of delivery is not guaranteed, for order-dependent functionality use streams.
	pub fn push_volatile_parcel(&mut self, parcel: P) -> Result<(), ConnectionError> {
		todo!()
	}

	/// Attempt to read data from the connection stream into the provided buffer.
	///
	/// # Returns
	/// Number of bytes read.
	///
	/// # Streams
	/// Connection streams offer
	/// [TCP](https://en.wikipedia.org/wiki/Transmission_Control_Protocol)-like functionality for
	/// contiguous streams of data. Streams are transmitted with the same network packets as
	/// reliable parcels, reducing overall data duplication for lost packets.
	///
	/// # Note
	/// Has consuming behavior, meaning repeated invocations will read exhaust internal stream
	/// buffer.
	pub fn read_from_stream(&mut self, buffer: &mut [u8]) -> Result<usize, ConnectionError> {
		todo!()
	}

	/// Write a given slice of bytes to the connection stream.
	///
	/// # Streams
	/// Connection streams offer
	/// [TCP](https://en.wikipedia.org/wiki/Transmission_Control_Protocol)-like functionality
	/// for contiguous streams of data. Streams are transmitted with the same network packets
	/// as reliable parcels, reducing overall data duplication for lost packets.
	pub fn write_bytes_to_stream(&mut self, bytes: &[u8]) -> Result<(), ConnectionError> {
		todo!()
	}

	/// Build the next packet that should be sent for this connection.
	///
	/// The connection must be in [`Open`](ConnectionStatus::Open) state!
	pub fn build_packet(&mut self, buffer: &mut [u8]) -> Result<usize, BuildPacketError> {
		todo!()
	}

	/// Build a connection-requesting packet that contains provided payload.
	///
	/// The connection must be in [`Pending`](ConnectionStatus::Pending) state!
	pub fn build_request_packet(&mut self, buffer: &mut [u8], payload: &[u8]) -> Result<usize, BuildPacketError> {
		todo!()
	}
}
