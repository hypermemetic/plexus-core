# Implementing a Plexus RPC Microservice — Index

> Comprehensive guide for AI agents and developers building services on the Plexus RPC stack.
> Split into focused documents for precise context loading.

## Documents

| Doc | Description |
|-----|-------------|
| [Plexus RPC Overview](16673909740283808511_plexus-rpc-overview.md) | What Plexus RPC is, architecture layers, dependencies, error handling, quickstart checklist, source paths |
| [Activation Trait and Macros](16673909740282808511_activation-trait-and-macros.md) | The `Activation` trait, `#[hub_methods]`/`#[hub_method]` macros, plugin ID, echo plugin walkthrough |
| [DynamicHub Routing](16673909740281808511_dynamichub-routing.md) | Registration (`.register()` vs `.register_hub()`), introspection methods, real-world builder example |
| [Server Setup and Transport](16673909740280808511_server-setup-and-transport.md) | `TransportServer` builder, WebSocket/stdio/MCP HTTP transports, RPC converter, complete `main.rs` |
| [Streaming Architecture](16673909740279808511_streaming-architecture.md) | Caller-wraps pattern, `PlexusStreamItem` variants, streaming helpers, bidirectional streaming |
| [Schema System](16673909740278808511_schema-system.md) | `PluginSchema`, `MethodSchema`, content hashing, `ChildSummary`, collision validation |
| [Advanced Patterns](16673909740277808511_advanced-patterns.md) | Parent context injection (`Arc::new_cyclic`), bidirectional communication, hub activations |
| [Synapse Client Layer](16673909740276808511_synapse-client-layer.md) | Request lifecycle, progressive discovery, IR system, parameter parsing, template rendering, backend discovery, bidirectional modes, effect stack |

## Reading Order

**Building a new microservice:** Overview → Activation → DynamicHub → Server Setup → Streaming

**Understanding the protocol:** Overview → Schema → Streaming → Synapse

**Advanced integration:** Advanced Patterns → Synapse (bidirectional section)

**Code generation / tooling:** Synapse (IR system section)
