//! Message-based networking over UDP for real-time applications.
//!
//! The library consists of 3 main modules:
//!
//! - [byte](byte) that handles byte-serialization.
//! - [endpoint](endpoint) that handles endpoint traits and implementations.
//! - [protocol](protocol) that implements high level functionality on top of endpoints.
//!
//! ## Features
//!
//! - `protocol` (default) - enables the [`protocol`](protocol) module. Users may opt-out if
//! they with to only use endpoint or byte-serialization portions of the library.
//! - `adv-endpoint` - advanced endpoint implementations. Their use is encouraged over using
//! default library [`endpoint`](endpoint) trait implementors, as the focus was simplicity
//! instead of performance.

#![warn(clippy::all)]

pub mod byte;
// pub mod endpoint;
pub mod connection;
