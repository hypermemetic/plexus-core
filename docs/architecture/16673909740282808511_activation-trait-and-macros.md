# Activation Trait and Macros

> How to define a Plexus RPC activation using `plexus-core` and `plexus-macros`.

## The Activation Trait

Every Plexus microservice is built from types implementing `Activation` (defined in `plexus-core/src/plexus/plexus.rs`):

```rust
#[async_trait]
pub trait Activation: Send + Sync + 'static {
    type Methods: MethodEnumSchema;

    fn namespace(&self) -> &str;
    fn version(&self) -> &str;
    fn description(&self) -> &str;         // Max 15 words
    fn long_description(&self) -> Option<&str>;
    fn methods(&self) -> Vec<&str>;
    fn method_help(&self, method: &str) -> Option<String>;

    /// Deterministic UUID from namespace@major_version
    fn plugin_id(&self) -> uuid::Uuid;

    /// Dispatch a method call, returning a stream of events
    async fn call(&self, method: &str, params: Value) -> Result<PlexusStream, PlexusError>;

    /// Optional: resolve a handle to a stream (for cross-plugin references)
    async fn resolve_handle(&self, handle: &Handle) -> Result<PlexusStream, PlexusError>;

    /// Convert to jsonrpsee Methods for transport registration
    fn into_rpc_methods(self) -> Methods where Self: Sized;

    /// Return this activation's schema (auto-generated from methods)
    fn plugin_schema(&self) -> PluginSchema;
}
```

You almost never implement this manually. The `#[hub_methods]` macro generates it all.

## Plugin ID

Deterministically generated from `namespace@major_version`:
```rust
uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, "echo@1".as_bytes())
```
This ensures handles survive minor/patch version upgrades.

## The `#[hub_methods]` Macro

Applied to an impl block, generates the complete `Activation` implementation:

```rust
#[plexus_macros::hub_methods(
    namespace = "echo",
    version = "1.0.0",
    description = "Echo messages back - demonstrates plexus-macros usage"
)]
impl Echo {
    // methods go here
}
```

**Attributes:**
- `namespace` — unique identifier, becomes the path segment
- `version` — semver string
- `description` — max 15 words, shown in discovery
- `long_description` — optional, for detailed docs
- `plugin_id` — optional UUID override (default: deterministic from namespace@major)

**Generated code:**
- `EchoMethod` enum for method dispatch
- `EchoRpc` trait with JSON-RPC subscription methods
- `EchoRpcServer` implementation
- Full `impl Activation for Echo` with schema, call routing, etc.
- Content hashes for each method (cache invalidation)

## The `#[hub_method]` Macro

Applied to individual methods within a `#[hub_methods]` block:

```rust
#[plexus_macros::hub_method(
    description = "Echo a message back the specified number of times",
    params(
        message = "The message to echo",
        count = "Number of times to repeat (default: 1)"
    )
)]
async fn echo(
    &self,
    message: String,
    count: u32,
) -> impl Stream<Item = EchoEvent> + Send + 'static {
    stream! {
        yield EchoEvent::Echo { message, count: 1 };
    }
}
```

**Method return requirements:**
- Must return `impl Stream<Item = T> + Send + 'static`
- `T` must implement `Serialize + Send`
- Use `async_stream::stream!` macro for the cleanest syntax

## Minimal Example: Echo Plugin

Two files:

### 1. Event types (`types.rs`)

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Events from echo operations
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EchoEvent {
    Echo {
        message: String,
        count: u32,
    },
}
```

**Critical details:**
- `#[serde(tag = "type", rename_all = "snake_case")]` — internally tagged enum, required for schema generation and Synapse IR
- `JsonSchema` derive — required for automatic JSON schema generation
- These are plain domain types. No special traits beyond Serialize + JsonSchema.

### 2. Activation (`activation.rs`)

```rust
use super::types::EchoEvent;
use async_stream::stream;
use futures::Stream;
use std::time::Duration;

#[derive(Clone)]
pub struct Echo;

impl Echo {
    pub fn new() -> Self { Echo }
}

#[plexus_macros::hub_methods(
    namespace = "echo",
    version = "1.0.0",
    description = "Echo messages back - demonstrates plexus-macros usage"
)]
impl Echo {
    #[plexus_macros::hub_method(
        description = "Echo a message back the specified number of times",
        params(
            message = "The message to echo",
            count = "Number of times to repeat (default: 1)"
        )
    )]
    async fn echo(
        &self,
        message: String,
        count: u32,
    ) -> impl Stream<Item = EchoEvent> + Send + 'static {
        let count = if count == 0 { 1 } else { count };
        stream! {
            for i in 0..count {
                if i > 0 {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                yield EchoEvent::Echo {
                    message: message.clone(),
                    count: i + 1,
                };
            }
        }
    }

    #[plexus_macros::hub_method(
        description = "Echo a message once",
        params(message = "The message to echo")
    )]
    async fn once(&self, message: String) -> impl Stream<Item = EchoEvent> + Send + 'static {
        stream! {
            yield EchoEvent::Echo { message, count: 1 };
        }
    }
}
```

## Source Paths

| Component | Path |
|-----------|------|
| Activation trait + DynamicHub | `plexus-core/src/plexus/plexus.rs` |
| hub_methods macro entry | `plexus-macros/src/lib.rs` |
| Activation codegen | `plexus-macros/src/codegen/activation.rs` |
| Method enum codegen | `plexus-macros/src/codegen/method_enum.rs` |
| Echo plugin (reference) | `plexus-substrate/src/activations/echo/` |
| Health plugin (manual impl) | `plexus-substrate/src/activations/health/` |
| Bash plugin (real-world) | `plexus-substrate/src/activations/bash/` |
