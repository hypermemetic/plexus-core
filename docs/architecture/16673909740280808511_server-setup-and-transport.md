# Server Setup and Transport

> How to configure and run a Plexus RPC server with `plexus-transport`.

## Complete `main.rs`

```rust
use plexus_core::plexus::DynamicHub;
use plexus_transport::TransportServer;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // Build hub with your activations
    let hub = Arc::new(
        DynamicHub::new("myservice")
            .register(Echo::new())
    );

    // RPC converter (required for WebSocket/stdio transports)
    let rpc_converter = |arc| {
        DynamicHub::arc_into_rpc_module(arc)
            .map_err(|e| anyhow::anyhow!("RPC error: {}", e))
    };

    // Start server
    TransportServer::builder(hub, rpc_converter)
        .with_websocket(4444)          // JSON-RPC over WebSocket
        // .with_stdio()               // OR stdio for MCP
        // .with_mcp_http(4445)        // Optional MCP HTTP+SSE
        .build().await?
        .serve().await
}
```

## Transport Options

| Transport | Method | Use case |
|-----------|--------|----------|
| `.with_websocket(port)` | WebSocket | Primary transport, bidirectional |
| `.with_stdio()` | Stdio | MCP-compatible, for tool integration |
| `.with_mcp_http(port)` | HTTP+SSE | MCP Streamable HTTP transport |

Stdio and WebSocket are mutually exclusive as primary. MCP HTTP can run alongside WebSocket.

## TransportServer Builder

The builder pattern from `plexus-transport/src/server.rs`:

```rust
TransportServer::builder(activation, rpc_converter)
    .with_websocket(port)           // Enable WebSocket on port
    .with_stdio()                   // Enable stdio (MCP-compatible)
    .with_mcp_http(port)            // Enable MCP HTTP+SSE on port
    .with_mcp_http_config(config)   // MCP HTTP with custom config
    .with_mcp_flat_schemas(schemas) // Pre-computed schemas for MCP tool exposure
    .with_mcp_route_fn(route_fn)    // Routing function for MCP call_tool dispatch
    .build().await?
    .serve().await
```

## RPC Converter

The `rpc_converter` function transforms `Arc<Activation>` into a `jsonrpsee::RpcModule`. For `DynamicHub`:

```rust
let rpc_converter = |arc| {
    DynamicHub::arc_into_rpc_module(arc)
        .map_err(|e| anyhow::anyhow!("Failed to create RPC module: {}", e))
};
```

This preserves the `Arc` lifecycle and `Weak` references for cross-plugin communication.

## Full Reference: Substrate `main.rs`

The reference implementation in `plexus-substrate/src/main.rs` shows a production setup with:
- CLI argument parsing (clap)
- Tracing/logging configuration
- Conditional transport selection (stdio vs WebSocket)
- MCP HTTP alongside WebSocket
- Activation info logging at startup

## Source Paths

| Component | Path |
|-----------|------|
| TransportServer builder | `plexus-transport/src/server.rs` |
| WebSocket transport | `plexus-transport/src/websocket.rs` |
| Stdio transport | `plexus-transport/src/stdio.rs` |
| MCP HTTP transport | `plexus-transport/src/mcp/` |
| Substrate main.rs | `plexus-substrate/src/main.rs` |
