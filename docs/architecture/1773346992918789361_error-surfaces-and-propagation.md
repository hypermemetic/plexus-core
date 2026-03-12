# Error Surfaces and Propagation

## Status

Planned — not yet implemented.

## Problem

Errors in the plexus stack can originate at two distinct points in the
call lifecycle, and each point has a different wire surface. Currently
the boundary between them is not clearly modelled, leading to:

- All `PlexusError` variants mapping to the same JSON-RPC code (`-32000`)
  regardless of what actually went wrong
- Panics inside stream generators bypassing both surfaces entirely,
  surfacing as jsonrpsee's anonymous `-32603 "Internal error"` with no
  method context
- `wrap_stream` calling `.expect("serialization failed")` which is itself
  a panic risk inside the streaming path
- No canonical function for converting `PlexusError` → `ErrorObjectOwned`,
  so the mapping is duplicated at two call sites in `plexus.rs` and
  inconsistent

## Two Error Surfaces

### Surface 1 — Pre-stream (subscription setup)

`route()` returns `Result<PlexusStream, PlexusError>`. The subscription
handler in `plexus.rs` propagates an `Err` as a JSON-RPC error response
before the client receives a subscription ID. The client sees this as
the subscription failing outright.

This surface already works. The only problem is the error information
that reaches the client is poor: wrong codes, no method context, no
structured data.

### Surface 2 — In-stream (subscription active)

Once the client holds a subscription ID, errors arrive as
`PlexusStreamItem::Error` items — stream notifications, not JSON-RPC
errors. `MethodInvoker` in plexus-gamma already handles this item type.
The helpers `error_stream()` and `error_stream_with_code()` in
`streaming.rs` exist to create these streams.

This surface works when code explicitly yields or returns an error
stream. The problem is getting there without `.unwrap()`.

### Why Panics Bypass Both

A panic inside an `async_stream::stream! { ... }` generator unwinds the
future without yielding anything. jsonrpsee catches the task panic at the
executor level and synthesises `-32603 "Internal error"` — a fixed string
with no method name, no variant, no context. Neither surface is involved.

## The `wrap_stream` Constraint

Activations fall into two patterns:

**Pattern A — typed domain events**

```rust
let stream = async_stream::stream! { yield MyEvent { ... }; };
Ok(wrap_stream(stream, "my.event", provenance))
```

The inner stream is `Stream<Item = T>`. `wrap_stream` maps each `T` to
`PlexusStreamItem::Data` via `serde_json::to_value(item).expect(...)`.
Pattern A cannot yield errors inline — there is no `PlexusStreamItem`
in scope, and the `.expect` inside `wrap_stream` is itself a panic risk.

**Pattern B — direct PlexusStreamItem**

```rust
Box::pin(stream! {
    yield PlexusStreamItem::Data { ... };
    yield PlexusStreamItem::Error { ... }; // already possible
})
```

Pattern B can already yield errors, but requires constructing
`PlexusStreamItem` manually which is verbose and couples the activation
to the transport type.

## Planned Solution

### 1. Canonical error conversion at the RPC boundary

Add a single function that converts `PlexusError` to `ErrorObjectOwned`
with correct JSON-RPC codes and structured `data`:

```rust
// plexus.rs (internal)
fn plexus_error_to_rpc(method: &str, e: PlexusError) -> ErrorObjectOwned {
    let (code, msg, data) = match e {
        PlexusError::ActivationNotFound(name) => (
            -32601,
            format!("Activation '{}' not found", name),
            json!({ "type": "activation_not_found", "activation": name, "method": method }),
        ),
        PlexusError::MethodNotFound { activation, method: m } => (
            -32601,
            format!("Method '{}.{}' not found", activation, m),
            json!({ "type": "method_not_found", "activation": activation, "method": m }),
        ),
        PlexusError::InvalidParams(msg) => (
            -32602,
            format!("Invalid params: {}", msg),
            json!({ "type": "invalid_params", "method": method, "detail": msg }),
        ),
        PlexusError::ExecutionError(msg) => (
            -32603,
            format!("Execution error: {}", msg),
            json!({ "type": "execution_error", "method": method, "detail": msg }),
        ),
        e => (-32000, e.to_string(), json!({ "type": "unknown", "method": method })),
    };
    ErrorObjectOwned::owned(code, msg, Some(data))
}
```

Both `.map_err()` call sites in `plexus.rs` become:

```rust
.map_err(|e| plexus_error_to_rpc(&p.method, e))?
```

### 2. Panic catching for the pre-stream path

Wrap `route()` in a `tokio::task::spawn` to convert task panics into
a readable error rather than jsonrpsee's anonymous `-32603`:

```rust
let method_name = p.method.clone();
let stream = match tokio::task::spawn({
    let plexus = plexus.clone();
    let method  = p.method.clone();
    let params  = p.params.unwrap_or_default();
    async move { plexus.route(&method, params).await }
}).await {
    Ok(Ok(s))  => s,
    Ok(Err(e)) => return Err(plexus_error_to_rpc(&method_name, e)),
    Err(e) if e.is_panic() => return Err(ErrorObjectOwned::owned(
        -32603,
        format!("Plugin panicked in '{}'", method_name),
        Some(json!({ "type": "panic", "method": method_name })),
    )),
    Err(e) => return Err(ErrorObjectOwned::owned(
        -32603,
        format!("Task error in '{}': {}", method_name, e),
        Some(json!({ "type": "task_error", "method": method_name })),
    )),
};
```

### 3. `wrap_result_stream` for Pattern A activations

Add a variant of `wrap_stream` that accepts `Stream<Item = Result<T, E>>`
and maps `Err(e)` to `PlexusStreamItem::Error` instead of panicking:

```rust
pub fn wrap_result_stream<T, E>(
    stream: impl Stream<Item = Result<T, E>> + Send + 'static,
    content_type: &'static str,
    provenance: Vec<String>,
) -> PlexusStream
where
    T: Serialize + Send + 'static,
    E: std::error::Error + Send + 'static,
```

Activations switch from `stream!` to `async_stream::try_stream!` so that
`?` inside the generator terminates the stream with an `Err` item:

```rust
// before
let stream = stream! {
    let config = load_config().unwrap();
    yield MyEvent { data: config };
};
Ok(wrap_stream(stream, "my.event", provenance))

// after
let stream = async_stream::try_stream! {
    let config = load_config()?;
    yield MyEvent { data: config };
};
Ok(wrap_result_stream(stream, "my.event", provenance))
```

### 4. Fix the existing panic in `wrap_stream`

`serde_json::to_value(item).expect("serialization failed")` in
`wrap_stream` is replaced with a `flat_map` that emits a
`PlexusStreamItem::Error` on serialization failure rather than panicking.
This is rare in practice (all activation event types derive `Serialize`)
but is nevertheless a correctness gap.

### 5. Mutex `.unwrap()` calls — leave as-is

```rust
self.inner.registry.read().unwrap()
self.inner.pending_rpc.lock().unwrap()
```

These only panic on mutex poisoning, which means another thread has
already panicked while holding the lock. The process is already in an
unrecoverable state. Replacing these with `?` would only obscure the
original panic. Leave them with `.unwrap()` or `.expect("mutex poisoned")`
for clarity.

## Error Code Mapping

| `PlexusError` variant     | JSON-RPC code | Rationale                          |
|---------------------------|---------------|------------------------------------|
| `ActivationNotFound`      | -32601        | The namespace does not exist       |
| `MethodNotFound`          | -32601        | The method does not exist          |
| `InvalidParams`           | -32602        | Params failed validation           |
| `ExecutionError`          | -32603        | Internal failure during execution  |
| `HandleNotSupported`      | -32000        | Custom — not a standard RPC case   |
| `TransportError`          | -32000        | Custom — transport-level failure   |
| Panic (via spawn)         | -32603        | Internal — process-level failure   |

## Work Items

1. Add `plexus_error_to_rpc(method, e)` to `plexus.rs`
2. Replace both `.map_err()` sites to use it
3. Wrap `route()` in `tokio::task::spawn` for panic catching
4. Add `wrap_result_stream` to `streaming.rs`
5. Fix `wrap_stream` serialization `.expect()` with `flat_map`
6. Audit activations in `plexus-core`, `substrate`, and any other crates
   for `.unwrap()` / `.expect()` calls in stream generators; migrate to
   `try_stream!` + `wrap_result_stream`
