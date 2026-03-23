# Changelog

All notable changes to hub-core will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Debug Endpoints**: Four built-in debug endpoints for protocol validation and troubleshooting
  - Enabled via `PLEXUS_DEBUG=true` environment variable
  - `_debug.protocol_test` - Validates streaming protocol implementation (Data, Progress, Done)
  - `_debug.stream_test` - Tests streaming scenarios (slow, large, many, progress)
  - `_debug.error_test` - Controlled error scenarios for error handling validation (immediate, after_data, recoverable)
  - `_debug.metadata_test` - Metadata edge cases and provenance chain testing
  - Auto-registered when `PLEXUS_DEBUG=true` is set at server startup
  - See DEBUGGING.md for comprehensive usage guide
  - Implementation in `src/plexus/debug.rs`
  - **Security**: Debug endpoints should never be enabled in production environments

### Changed

- **BREAKING**: `DynamicHub::new()` now requires explicit namespace parameter
  - Forces intentional naming instead of defaulting to "plexus"
  - Example: `DynamicHub::new("substrate")` or `DynamicHub::new("myapp")`
  - Rationale: DynamicHub is a composition tool - its namespace should reflect your application

- **DEPRECATION**: `Plexus` type renamed to `DynamicHub` to clarify architecture
  - `Plexus` remains as a deprecated type alias for backwards compatibility
  - Will be removed in a future major version
  - Rationale: "Plexus" implied special infrastructure, when it's actually just an `Activation` with dynamic registration
  - See architecture documentation for migration guide

### Migration Guide

Replace `Plexus::new()` with `DynamicHub::new(namespace)`:

```rust
// Before
use hub_core::plexus::Plexus;
let hub = Plexus::new().register(activation);

// After
use hub_core::plexus::DynamicHub;
let hub = DynamicHub::new("myapp").register(activation);
```

Choose a namespace that identifies your application:
- "substrate" for substrate server
- "hub" for generic hubs
- "myapp" for your application name

The `Plexus` type alias will continue to work but will show deprecation warnings.
The old `with_namespace()` method is deprecated in favor of `new(namespace)`.

## [0.2.1] - Previous releases

See git history for earlier changes.
