//! HubContext - generic parent context injection for hub-aware plugins
//!
//! This module defines the `HubContext` trait that allows plugins to receive
//! typed context from their parent hub. The pattern enables:
//!
//! - Symmetric context passing: every hub passes something to its children
//! - Generic plugins: `Plugin<P: HubContext>` works with any parent type
//! - Type safety: children know what capabilities their parent provides
//!
//! # Example
//!
//! ```ignore
//! // A plugin generic over its parent context
//! pub struct Cone<P: HubContext> {
//!     parent: Arc<OnceLock<P>>,
//!     storage: Arc<ConeStorage>,
//! }
//!
//! impl<P: HubContext> Cone<P> {
//!     pub fn inject_parent(&self, parent: P) {
//!         let _ = self.parent.set(parent);
//!     }
//!
//!     async fn resolve_foreign_handle(&self, handle: &Handle) -> Option<Value> {
//!         self.parent.get()?.resolve_handle(handle).await
//!     }
//! }
//!
//! // Plexus injects itself (as Weak<Plexus>)
//! let cone: Cone<Weak<Plexus>> = Cone::new();
//! cone.inject_parent(Arc::downgrade(&plexus));
//!
//! // A sub-hub could inject its own context
//! let widget: Widget<DashboardContext> = Widget::new();
//! widget.inject_parent(dashboard_ctx);
//! ```

use crate::plexus::streaming::PlexusStream;
use crate::plexus::PlexusError;
use crate::types::Handle;
use async_trait::async_trait;

/// Trait for parent context that can be injected into child plugins.
///
/// This trait defines the capabilities a child plugin expects from its parent.
/// Any hub that wants to inject context into children should implement this trait
/// (or provide a type that implements it).
///
/// The trait is object-safe to allow dynamic dispatch when needed, though
/// most usage will be through generic bounds.
#[async_trait]
pub trait HubContext: Clone + Send + Sync + 'static {
    /// Resolve a handle through the parent's plugin registry.
    ///
    /// This allows children to resolve handles from sibling plugins without
    /// knowing about them directly - they just ask the parent to route it.
    async fn resolve_handle(&self, handle: &Handle) -> Result<PlexusStream, PlexusError>;

    /// Route a method call through the parent.
    ///
    /// Enables children to call methods on sibling plugins via the parent hub.
    async fn call(&self, method: &str, params: serde_json::Value) -> Result<PlexusStream, PlexusError>;

    /// Check if the parent context is still valid.
    ///
    /// For weak references, this checks if the parent is still alive.
    /// For strong references or owned contexts, this always returns true.
    fn is_valid(&self) -> bool {
        true
    }
}

/// Marker trait for plugins that accept parent context injection.
///
/// Plugins that implement this trait can receive context from their parent hub
/// after construction. The associated `Parent` type specifies what kind of
/// context the plugin expects.
pub trait ParentAware {
    /// The type of parent context this plugin expects.
    type Parent: HubContext;

    /// Inject the parent context.
    ///
    /// Called by the parent hub after plugin construction.
    /// Typically stores the context in an `OnceLock` for later use.
    fn inject_parent(&self, parent: Self::Parent);

    /// Check if parent has been injected.
    fn has_parent(&self) -> bool;
}

/// A no-op context for plugins that don't need parent access.
///
/// This allows plugins to be instantiated without a parent, useful for
/// testing or standalone operation.
#[derive(Clone, Debug, Default)]
pub struct NoParent;

#[async_trait]
impl HubContext for NoParent {
    async fn resolve_handle(&self, _handle: &Handle) -> Result<PlexusStream, PlexusError> {
        Err(PlexusError::ExecutionError(
            "No parent context available for handle resolution".to_string(),
        ))
    }

    async fn call(&self, method: &str, _params: serde_json::Value) -> Result<PlexusStream, PlexusError> {
        Err(PlexusError::ExecutionError(format!(
            "No parent context available to route call: {}",
            method
        )))
    }

    fn is_valid(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_parent_is_invalid() {
        let ctx = NoParent;
        assert!(!ctx.is_valid());
    }

    #[tokio::test]
    async fn no_parent_resolve_fails() {
        let ctx = NoParent;
        let handle = Handle::new(uuid::Uuid::new_v4(), "1.0.0", "test");
        let result = ctx.resolve_handle(&handle).await;
        assert!(result.is_err());
    }
}
