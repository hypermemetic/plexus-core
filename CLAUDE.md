Often I will say "open this" in reference to code use `codium --goto` to take me there

if you need to see the dir structure use the `tre` command

## Architecture Documentation Naming Convention

Architecture documents in `docs/architecture/` use reverse-chronological naming to ensure newest documents appear first in alphabetical sorting.

**Naming formula**: `(u64::MAX - nanotime)_title.md`

Where:
- `nanotime` = current Unix timestamp in nanoseconds
- This creates a descending numeric prefix (newer = smaller number = sorts first)
- Example: `16681577588676290559_type-system.md`

**To generate a filename**:
```python
import time
nanotime = int(time.time() * 1_000_000_000)
filename = (2**64 - 1) - nanotime
print(f'{filename}_your-title.md')
```

This "chronological bubbling" helps prioritize recent architectural decisions.


This project consists of a few pieces: Substrate, Symbols, and Cllient. All should be in the parent which should be called `controlflow`
When I say "create an architecture doc and link it" I mean to create the same file in whichever subprojects it makes sense to add to. We then have a claude instance which reads them there and this steers cross project development. Ideally when I say "write a doc" that implicitly includes linking it and also mentioning it in the commit you add it to. usually we write a doc when we are going to commit anyway. if I'm asking for one determine if this is at a commit boundary

## Planning Documents Convention

Planning docs live in `plans/<EPIC>/` where EPIC is a short prefix (e.g., MCP, ARBOR, CONE).

**Structure:**
```
plans/
  MCP/
    MCP-1.md   # Epic overview (the "master plan")
    MCP-2.md   # Individual ticket
    MCP-3.md   # Individual ticket
    ...
```

**Ticket format:** `<EPIC>-<N>.md` where N is sequential within the epic.

**Epic overview (X-1.md)** contains:
- High-level goal and context
- Dependency DAG showing which tickets unlock others
- Phase breakdown

**Individual tickets (X-N.md)** contain:
- `blocked_by: [X-2, X-3]` - tickets that must complete first
- `unlocks: [X-5, X-6]` - tickets that can start once this completes
- Scope, acceptance criteria, implementation notes

**Design for parallelism:** Structure tickets so completing one unlocks multiple concurrent tickets. The DAG should fan out, not be linear. Identify the critical path and minimize its length.

```
     X-2 (foundation)
      │
  ┌───┼───┬───┐
  ▼   ▼   ▼   ▼
 X-3 X-4 X-5 X-6  ← parallel work
  │   │   │   │
  └───┴───┼───┘
          ▼
        X-7 (integration)
```

When a ticket is completed, check its `unlocks` field to identify newly unblocked work.

---

## Plexus RPC

**Location**: `plexus-core/`, `plexus-macros/`, `plexus-transport/`, `plexus-substrate/`, `plexus-registry/` (in hypermemetic repo)
**Language**: Rust
**Protocol**: JSON-RPC 2.0 over WebSocket

### What It Is

Plexus is a streaming RPC framework where code IS schema. Proc macros extract API schemas directly from Rust function signatures -- no IDL files, no code generation step, no schema drift. Every Plexus server is self-describing and introspectable at runtime.

### Crate Dependency Graph

```
plexus-macros     -> Proc macros (#[hub_methods], #[hub_method]) generate Activation impls from Rust code
plexus-core       -> Core types: Activation trait, DynamicHub (router), ChildRouter, schema types, Handle system
plexus-transport  -> Server layer: WebSocket JSON-RPC, stdio transport, MCP HTTP+SSE
plexus-substrate  -> Reference server hosting multiple activations (arbor, cone, bash, hyperforge, etc.)
plexus-registry   -> Backend discovery service (SQLite+TOML), enables multi-backend routing
```

### Core Concepts

**Activation**: A unit of functionality implementing the `Activation` trait. Has a namespace, version, methods, and optionally children.

```rust
#[hub_methods(namespace = "echo", version = "1.0.0")]
impl Echo {
    #[hub_method]
    async fn once(&self, req: EchoRequest) -> impl Stream<Item = EchoEvent> { ... }
}
```

The proc macro generates: Activation trait impl, method enum with JSON Schema, RPC dispatch, schema() introspection method. The function signature IS the API contract.

**DynamicHub**: A router activation that composes child activations. `"arbor.tree_create"` splits on first dot, routes to `arbor` child, calls `tree_create`. DynamicHub is itself just an Activation -- no special infrastructure.

**Streaming-first**: Every method returns `impl Stream<Item = T>`. Response items are `PlexusStreamItem`:
- `Content { metadata, content_type, data }` -- actual response data
- `Progress { metadata, message, percentage }` -- progress updates
- `Error { metadata, message, code, recoverable }` -- error events
- `Done { metadata }` -- stream termination

Request-response semantics = single Content + Done.

**Content-hashed schemas**: Each method and plugin has a content hash. Parent hashes incorporate child hashes. Root hash changes when ANY descendant changes. Enables cache invalidation and drift detection.

**Hierarchical namespacing**: Activations can nest arbitrarily deep via ChildRouter trait. Method paths are dot-separated. Hub activations route to children; leaf activations contain only methods.

### Protocol

JSON-RPC 2.0 over WebSocket. Core RPC methods:
- `plexus_call` -- invoke a method: `{ "method": "namespace.method", "params": {...} }`
- `plexus_schema` -- get root hub schema (child schemas fetched lazily)
- `{namespace}.schema` -- get any activation's schema directly

### Schema System

```rust
PluginSchema {
    namespace, version, description,
    methods: Vec<MethodSchema>,        // name, description, params (JSON Schema), returns (JSON Schema), hash
    children: Option<Vec<ChildSummary>>, // summaries only, not recursive
    hash: String,                       // content hash
}
```

Child schemas are NOT embedded -- clients fetch them individually via lazy traversal. This keeps responses lightweight.

### Macro Rules

Method signatures parsed by `#[hub_method]`:
- Must be `async`
- First non-self param -> input schema (must impl `Deserialize + JsonSchema`)
- Return type must be `impl Stream<Item = EventType> + Send + 'static`
- Params named `ctx`/`context` are skipped (context injection)
- Doc comments become method descriptions in schema

### Discovery Pipeline

```
Rust types -> (plexus-macros) -> Runtime JSON Schema -> (synapse --emit-ir) -> IR -> (codegen) -> TS/Python/Rust clients
```

Synapse CLI (Haskell) connects to any Plexus server, discovers schema at runtime, emits IR for client code generation. The CLI writes itself from the API structure.

### Key Source Files

```
plexus-core/src/plexus/
  plexus.rs        -- Activation trait, DynamicHub, ChildRouter
  schema.rs        -- PluginSchema, MethodSchema types
  stream.rs        -- PlexusStreamItem, PlexusStream
  handle.rs        -- Handle types (typed references with provenance)
  bridge.rs        -- PlexusMcpBridge (MCP server integration)

plexus-macros/src/
  lib.rs           -- #[hub_methods] and #[hub_method] proc macro entry points

plexus-transport/src/
  websocket.rs     -- WebSocket JSON-RPC server
  stdio.rs         -- Stdio transport
  mcp.rs           -- MCP HTTP+SSE transport
```

### Design Principles

1. **Code IS Schema** -- Rust type signature defines the API contract. Zero drift by construction.
2. **Streaming First** -- All methods return streams. Request-response is a degenerate case.
3. **Hierarchical Composition** -- Activations route to children, enabling arbitrary depth without central registry.
4. **Identity via Content Hash** -- Schemas versioned by content, not arbitrary numbers.
5. **No Magic** -- DynamicHub is just an Activation. Routing is explicit. No hidden infrastructure.
