//! Transport-specific types for bidirectional communication.
//!
//! This module provides the wire-format types for different transports.
//! Each transport has its own module with adapted message types.

pub mod websocket;

pub use websocket::{ClientMessage, SubscriptionMessage};
