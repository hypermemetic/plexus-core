//! Example DynamicHub builder
//!
//! This module provides an example of how to build a DynamicHub instance.
//! Real applications should create their own builder with their specific activations.

use std::sync::Arc;

use crate::activations::echo::Echo;
use crate::activations::health::Health;
use crate::plexus::DynamicHub;

#[allow(deprecated)]
use crate::plexus::Plexus;

/// Build an example hub with minimal activations
///
/// This demonstrates how to construct a DynamicHub instance.
/// Real applications should define their own builder function
/// that registers their specific activations.
///
/// DynamicHub itself provides introspection methods:
/// - {namespace}.call: Route calls to registered activations
/// - {namespace}.hash: Get configuration hash for cache invalidation
/// - {namespace}.schema: Get plugin schemas
pub fn build_example_hub() -> Arc<DynamicHub> {
    Arc::new(
        DynamicHub::new("example")
            .register(Health::new())
            .register(Echo::new()),
    )
}

/// Deprecated: Use build_example_hub instead
#[deprecated(since = "0.3.0", note = "Use build_example_hub instead")]
pub fn build_example_plexus() -> Arc<Plexus> {
    build_example_hub()
}
