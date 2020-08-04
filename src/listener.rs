//! A listener listens for new connections, accepting or denying them.

use super::{Transmit, FilterConnection};

/// A listener passively listens for new connections.
/// 
/// The new connections are pending, letting the application decide whether to accept a particular new connection.
pub struct Listener<T: Transmit + FilterConnection> {
	endpoint: T,
}
