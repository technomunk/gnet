//! Implementations of [`Transmit`](super::Transmit) and [`Listen`](super::Listen)
//! traits for wrapping types (such as [`Arc`](std::sync::Arc)).

use std::boxed::Box;
use std::io::Error as IoError;
use std::net::SocketAddr;

use super::{Transmit, TransmitError, Listen};

use crate::id::ConnectionId;

macro_rules! impl_transmit_for {
	($wrapper:ty) => {
		impl<T: Transmit> Transmit for $wrapper {
			const PACKET_BYTE_COUNT: usize = T::PACKET_BYTE_COUNT;
			const RESERVED_BYTE_COUNT: usize = T::RESERVED_BYTE_COUNT;

			#[inline]
			fn send_to(&self, data: &mut [u8], addr: SocketAddr) -> Result<usize, IoError> {
				T::send_to(self, data, addr)
			}

			#[inline]
			fn recv_all(
				&self,
				buffer: &mut Vec<u8>,
				connection_id: ConnectionId
			) -> Result<usize, TransmitError> {
				T::recv_all(self, buffer, connection_id)
			}
		}
	};
}

impl_transmit_for!(std::sync::Arc<T>);

macro_rules! impl_listen_for {
	($wrapper:ty) => {
		impl<T: Listen> Listen for $wrapper {
			#[inline]
			fn allow_connection_id(&self, connection_id: ConnectionId) {
				T::allow_connection_id(self, connection_id)
			}

			#[inline]
			fn block_connection_id(&self, connection_id: ConnectionId) {
				T::block_connection_id(self, connection_id)
			}

			#[inline]
			fn pop_connectionless_packet(&self) -> Result<(SocketAddr, Box<[u8]>), TransmitError> {
				T::pop_connectionless_packet(self)
			}
		}
	};
}

impl_listen_for!(std::sync::Arc<T>);
