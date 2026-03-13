//! Hub Core - Core infrastructure for building Plexus RPC servers
//!
//! This crate provides:
//! - `DynamicHub` - Dynamic routing hub for activations (implements Plexus RPC protocol)
//! - `Activation` - Trait for implementing activations
//! - `PlexusMcpBridge` - MCP server integration
//! - `Handle` - Typed references to activation method results
//!
//! # Example
//!
//! ```rust,no_run
//! use plexus_core::plexus::DynamicHub;
//! use plexus_core::activations::echo::Echo;
//! use plexus_core::activations::health::Health;
//! use std::sync::Arc;
//!
//! let hub = Arc::new(
//!     DynamicHub::new("myapp")
//!         .register(Health::new())
//!         .register(Echo::new())
//! );
//! ```

pub mod activations;
pub mod builder;
pub mod mcp_bridge;
pub mod plexus;
pub mod plugin_system;
pub mod serde_helpers;
pub mod types;

// Re-export commonly used items
pub use builder::build_example_hub;
pub use mcp_bridge::PlexusMcpBridge;
pub use plexus::{Activation, DynamicHub, PlexusError, PLEXUS_NOTIF_METHOD};
pub use types::{Envelope, Handle, HandleParseError, HandleResolutionParams, Origin};
