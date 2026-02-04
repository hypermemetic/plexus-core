//! Global Plexus RPC context for sharing state with activations
//!
//! This module provides a thread-safe way to share the Plexus RPC server hash
//! with all activations. The hash is set once during hub initialization
//! and can be read by any activation when creating stream items.

use std::sync::OnceLock;

/// Global Plexus RPC context singleton
static PLEXUS_CONTEXT: OnceLock<PlexusContext> = OnceLock::new();

/// Shared Plexus RPC context accessible to all activations
#[derive(Debug, Clone)]
pub struct PlexusContext {
    /// Hash of all activations for cache invalidation
    pub hash: String,
}

impl PlexusContext {
    /// Initialize the global Plexus RPC context
    ///
    /// This should be called once during hub initialization.
    /// Subsequent calls will be ignored.
    pub fn init(hash: String) {
        let _ = PLEXUS_CONTEXT.set(PlexusContext { hash });
    }

    /// Get the global Plexus RPC server hash
    ///
    /// Returns an empty string if the context hasn't been initialized yet.
    pub fn hash() -> String {
        PLEXUS_CONTEXT
            .get()
            .map(|ctx| ctx.hash.clone())
            .unwrap_or_default()
    }
}
