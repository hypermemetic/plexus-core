# hub-core

Core infrastructure for building hub-based systems with dynamic routing.

## Overview

hub-core provides the foundation for building pluggable systems with hierarchical routing and schema introspection:

- **DynamicHub** - Dynamic routing hub for composing activations (formerly Plexus)
- **Activation** - Trait for implementing plugins
- **PlexusMcpBridge** - MCP server integration via rmcp
- **Handle** - Typed references to plugin method results
- **hub-macro** - Procedural macro for generating activation boilerplate

> **Note**: `Plexus` has been renamed to `DynamicHub` to clarify that it's an activation with dynamic registration, not special infrastructure. The `Plexus` type alias remains for backwards compatibility but is deprecated.

## Quick Start

```rust
use hub_core::plexus::DynamicHub;
use hub_core::{Activation, PlexusError};
use std::sync::Arc;

// Create a dynamic hub with explicit namespace and register your activations
let hub = Arc::new(
    DynamicHub::new("myapp")
        .register(MyActivation::new())
);

// Route calls to activations
let stream = hub.route("myactivation.method", json!({})).await?;
```

> **Migration Note**: `DynamicHub::new()` now requires an explicit namespace. Choose a namespace that identifies your application (e.g., "myapp", "substrate", "hub"). The `Plexus` type alias still works but is deprecated.

## Creating Activations

Use the `hub-macro` crate to generate activation implementations:

```rust
use hub_macro::hub_methods;
use async_stream::stream;

#[derive(Clone)]
pub struct MyApp;

#[hub_methods(
    namespace = "myapp",
    version = "1.0.0",
    description = "My application"
)]
impl MyApp {
    /// Say hello
    #[hub_method]
    async fn hello(&self, name: String) -> impl Stream<Item = MyEvent> + Send + 'static {
        stream! {
            yield MyEvent::Greeting { message: format!("Hello, {}!", name) };
        }
    }
}
```

## MCP Bridge

hub-core includes an MCP server bridge that exposes activations as MCP tools:

```rust
use hub_core::{DynamicHub, PlexusMcpBridge};

let hub = Arc::new(DynamicHub::new().register(MyApp::new()));
let bridge = PlexusMcpBridge::new(hub);

// Use with rmcp server
```

> Note: PlexusMcpBridge works with any activation, not just DynamicHub.

## Example Activations

hub-core includes two minimal example activations:

- **health** - Health check endpoint (manual Activation impl)
- **echo** - Echo messages back (hub-macro generated)

See `src/activations/` for implementation examples.

## License

AGPL-3.0-only
