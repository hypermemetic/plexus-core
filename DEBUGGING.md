# Debugging Plexus Applications

Guide to debugging Plexus RPC applications using built-in debug endpoints and tools.

## Debug Mode

### Enabling Debug Endpoints

Debug endpoints are enabled via the `PLEXUS_DEBUG` environment variable:

```bash
# Enable debug mode
export PLEXUS_DEBUG=true

# Run your Plexus backend
cargo run --bin substrate
# or
./your-plexus-backend
```

When enabled, you'll see this log message:
```
PLEXUS_DEBUG=true: Debug endpoints enabled
```

When disabled (default):
```
PLEXUS_DEBUG not set: Debug endpoints disabled
```

### Available Debug Endpoints

#### 1. Protocol Test (`_debug.protocol_test`)

Validates the complete Plexus streaming protocol by generating:
- StreamData items with test payloads
- StreamProgress items (50% progress)
- StreamDone completion

**Use case**: Verify client protocol implementation

**Example**:
```bash
synapse substrate _debug.protocol_test
```

**Expected output**:
```
data: Test message 1
progress: 50%
data: Test message 2
[StreamDone]
```

#### 2. Stream Test (`_debug.stream_test`)

Tests different streaming patterns to validate stream handling.

**Scenarios**:

- **slow** - 5 items with increasing delays (100-200ms)
  - Use case: Test client handling of delayed data
  - Expected behavior: Progressive output with noticeable delays

- **large** - 3 items with 10KB payloads each
  - Use case: Test client buffer handling
  - Expected behavior: Three large data items received correctly

- **many** - 100 items with minimal delay
  - Use case: Test high-volume streaming
  - Expected behavior: 100 items received quickly

- **progress** - Progress updates at 0%, 25%, 50%, 75%, 100%
  - Use case: Test progress reporting
  - Expected behavior: Five progress updates in sequence

**Examples**:
```bash
# Test slow streaming
synapse substrate _debug.stream_test --scenario slow

# Test large payload handling
synapse substrate _debug.stream_test --scenario large

# Test high-volume streaming
synapse substrate _debug.stream_test --scenario many

# Test progress reporting
synapse substrate _debug.stream_test --scenario progress
```

#### 3. Error Test (`_debug.error_test`)

Generates controlled error scenarios to test error handling.

**Error types**:

- **immediate** - Error before any data
  - Use case: Test client handling of immediate failures
  - Expected behavior: Error received, no data items

- **after_data** - Error after sending 3 data items
  - Use case: Test client handling of mid-stream errors
  - Expected behavior: 3 data items, then error

- **recoverable** - Recoverable error with stream continuation
  - Use case: Test error recovery mechanisms
  - Expected behavior: Error sent but stream continues

**Examples**:
```bash
# Test immediate error handling
synapse substrate _debug.error_test --error_type immediate

# Test error after data
synapse substrate _debug.error_test --error_type after_data

# Test recoverable errors
synapse substrate _debug.error_test --error_type recoverable
```

#### 4. Metadata Test (`_debug.metadata_test`)

Tests metadata edge cases including:
- Single provenance item
- Nested provenance chains (3 levels)
- Long provenance chains (6 items)
- Different plexus_hash values
- Different timestamps

**Use case**: Validate metadata parsing and provenance handling

**Example**:
```bash
synapse substrate _debug.metadata_test
```

**Expected output**: Stream with 5 test messages, each with different metadata structures

## Common Debugging Workflows

### Validate Client Protocol Implementation

```bash
# 1. Enable debug mode
export PLEXUS_DEBUG=true
./your-backend &

# 2. Test basic protocol
synapse substrate _debug.protocol_test

# 3. Verify all message types received correctly
# Look for: data, progress, done messages
```

### Test Stream Performance

```bash
# Test different load patterns
synapse substrate _debug.stream_test --scenario slow
synapse substrate _debug.stream_test --scenario many
synapse substrate _debug.stream_test --scenario large

# Monitor for:
# - Timeouts
# - Buffer overflows
# - Memory usage spikes
```

### Verify Error Handling

```bash
# Test error scenarios
synapse substrate _debug.error_test --error_type immediate
synapse substrate _debug.error_test --error_type after_data
synapse substrate _debug.error_test --error_type recoverable

# Check that client:
# - Receives error messages
# - Handles partial streams correctly
# - Recovers from errors when possible
```

### Debug Connection Issues

Use synapse's `_self debug` command for connection-level debugging:

```bash
synapse _self debug localhost 4444 substrate
```

This tests:
1. TCP connection
2. HTTP endpoint
3. WebSocket upgrade
4. Plexus RPC protocol

Then use debug endpoints to validate protocol compliance:

```bash
synapse _self validate localhost 4444 substrate
```

This runs a comprehensive protocol validation test suite using the debug endpoints.

### Test Specific Methods

Use synapse's `_self test` command to test arbitrary methods with protocol validation:

```bash
# Test with validated params
synapse _self test localhost 4444 substrate echo.echo --message "hello"

# Test with unknown params (server robustness)
synapse _self test --allow-unknown localhost 4444 substrate echo.echo \
  --message "hello" \
  --fake "test"

# Test with raw JSON (edge cases)
synapse _self test --raw '{"message":"hello"}' localhost 4444 substrate echo.echo
```

## Implementation Details

Debug endpoints are implemented in `src/plexus/debug.rs` and automatically registered in `src/plexus/plexus.rs` when the environment variable is detected.

### Auto-Registration Logic

```rust
// In src/plexus/plexus.rs (lines 989-1052)
if std::env::var("PLEXUS_DEBUG").is_ok() {
    tracing::info!("PLEXUS_DEBUG=true: Debug endpoints enabled");
    let debug_activation = Arc::new(debug::DebugActivation::new());
    rpc_module = rpc_module.merge_dyn(
        debug_activation
            .arc_into_rpc_module()
            .await
            .expect("Debug activation conversion failed")
    );
} else {
    tracing::info!("PLEXUS_DEBUG not set: Debug endpoints disabled");
}
```

All debug endpoints use the `_debug` namespace prefix to avoid conflicts with application namespaces.

### Debug Endpoint Implementation

Each endpoint is a standard Plexus RPC method that returns a `PlexusStream`:

```rust
#[hub_method(
    description = "Test basic protocol with all message types",
    params()
)]
async fn protocol_test(&self) -> impl Stream<Item = DebugEvent> + Send + 'static {
    stream! {
        yield DebugEvent::Message { ... };
        yield DebugEvent::Progress { ... };
        yield DebugEvent::Message { ... };
        // StreamDone automatically sent by framework
    }
}
```

## Security Notes

**CRITICAL**: Debug endpoints expose internal system behavior and should NEVER be enabled in production.

### Why Debug Mode is Dangerous

- **No authentication**: Debug endpoints bypass normal authorization
- **Information disclosure**: Exposes implementation details and internal state
- **Resource exhaustion**: Can generate high load (e.g., `stream_test` with "many" scenario)
- **Attack surface**: Provides additional entry points for potential exploits

### Production Safety Checklist

- [ ] `PLEXUS_DEBUG` is NOT set in production environment
- [ ] Production configs don't include debug mode
- [ ] CI/CD pipelines don't enable debug in production stages
- [ ] Environment variables are validated before deployment
- [ ] Monitoring alerts on unexpected debug endpoint calls (if possible)

### Why Environment Variable Activation?

**Alternatives considered**:
- ❌ Compile-time feature flags - Too inflexible, requires rebuilding
- ❌ Configuration files - Harder to secure, easier to accidentally commit
- ❌ CLI flags - Inconsistent across different deployment methods

**Environment variables are best because**:
- ✅ Easy to control in different environments
- ✅ Standard practice for debug/dev features
- ✅ Difficult to accidentally enable in production
- ✅ Logged at startup for visibility
- ✅ No code changes or rebuilds required

## Troubleshooting

### Debug Endpoints Not Available

**Symptom**: `synapse substrate _debug.protocol_test` returns error

**Possible causes**:
1. `PLEXUS_DEBUG` not set - Check server logs for "Debug endpoints disabled"
2. Server needs restart - Debug mode is checked at startup
3. Wrong namespace - Ensure `substrate` matches your server's namespace

**Solution**:
```bash
# Set environment variable
export PLEXUS_DEBUG=true

# Restart server
./your-backend

# Verify in logs
# Should see: "PLEXUS_DEBUG=true: Debug endpoints enabled"
```

### Debug Test Timeouts

**Symptom**: Debug endpoint calls hang or timeout

**Possible causes**:
1. Server not sending StreamDone
2. Client not handling streams correctly
3. Network issues

**Solution**:
```bash
# Use synapse's protocol validator
synapse _self validate localhost 4444 substrate

# Check for specific violations:
# - Missing StreamDone
# - Incorrect metadata structure
# - Field naming issues
```

### High Memory Usage During Debug Tests

**Symptom**: Server memory spikes when running debug tests

**Possible causes**:
1. `stream_test` with "many" scenario (100 items)
2. `stream_test` with "large" scenario (10KB payloads)

**Solution**: This is expected behavior for load testing. Use lighter scenarios for routine debugging:
```bash
# Use slow scenario instead of many
synapse substrate _debug.stream_test --scenario slow

# Or test basic protocol instead
synapse substrate _debug.protocol_test
```

## Related Documentation

- **README.md** - Overview of hub-core and quick start
- **src/plexus/debug.rs** - Debug endpoint source code
- **synapse README.md** - Client-side debugging with `_self` commands
- **integration-tests/EDGE_MATRIX.md** - Integration test suite including debug endpoint tests

## Future Enhancements

Potential additions to debug system:

- **_debug.subscriptions** - Show active subscription count and details
- **_debug.metrics** - Performance metrics snapshot (latency, throughput)
- **_debug.schema_all** - Recursive schema dump of all registered activations
- **Structured logging control** - Per-endpoint logging level configuration
- **Trace correlation** - Include trace IDs in debug responses

See `src/plexus/debug.rs` source for current implementation and inline documentation.
