# Plexus → DynamicHub Rename

**Date**: 2026-01-24
**Status**: Proposed
**Decision**: Rename `Plexus` to `DynamicHub` and clarify activation hosting model

## Problem Statement

The name "Plexus" creates confusion about the architecture:

1. **Implies special infrastructure** - Suggests Plexus is required framework code
2. **Namespace collision** - Default namespace is "plexus", same as type name
3. **Obscures the pattern** - Hides that ANY activation can be a hub
4. **Misleading examples** - Makes it seem like you must use Plexus

### The Core Insight

**ANY Activation can route to children.** Plexus is NOT special infrastructure—it's just an Activation with dynamic registration.

**Proof: Solar**

```rust
// Solar IS a hub activation
impl Activation for Solar {
    async fn call(&self, method: &str, params) -> Result<PlexusStream> {
        // Routes "mercury.info" → Mercury → info()
    }
}

impl ChildRouter for Solar {
    async fn get_child(&self, name: &str) -> Option<Box<dyn ChildRouter>> {
        // Returns Mercury, Venus, Earth, etc.
    }
}
```

Solar implements `Activation`, routes to children (planets), and can be hosted directly:

```rust
// Host Solar directly - NO Plexus needed!
TransportServer::builder(solar, converter).serve().await?;

// Calls work: solar.mercury.info, solar.earth.luna.info
```

**Solar is a hub.** Plexus is also a hub. The only difference is:
- **Solar**: Hardcoded children (planets built at construction)
- **Plexus**: Dynamic children (`.register()` at runtime)

Both are just `Activation` implementations that happen to route.

## The Confusion

Current architecture suggests this hierarchy:

```
Plexus (special framework)
  └─ Activations (your code)
      └─ Solar (nested hub)
          └─ Mercury (leaf)
```

But the reality is:

```
Activation (trait)
  ├─ Echo (leaf - no routing)
  ├─ JsExec (leaf - no routing)
  ├─ Solar (hub - hardcoded children)
  └─ Plexus (hub - dynamic children)
```

**They're all just Activations.** Some route, some don't. None are "special."

## Proposed Solution

### 1. Rename `Plexus` to `DynamicHub`

```rust
// Before
pub struct Plexus { ... }
let plexus = Plexus::new()
    .register(Solar::new())
    .register(Echo::new());

// After
pub struct DynamicHub { ... }
let hub = DynamicHub::new("myapp")  // Requires explicit namespace
    .register(Solar::new())
    .register(Echo::new());
```

**Why "DynamicHub"?**
- **Dynamic** - Children registered at runtime via `.register()`
- **Hub** - Routes to multiple child activations
- **Clear** - Describes what it does, not a project name

**Why require explicit namespace?**
- DynamicHub is a composition tool - its namespace should reflect your application
- Unlike single activations which have fixed namespaces
- Forces intentional naming instead of defaulting to "plexus"
- Example namespaces: "substrate", "myapp", "hub"

### 2. Emphasize Direct Activation Hosting

**DynamicHub is OPTIONAL. Any activation can be hosted directly.**

#### Single Activation (Recommended Pattern)

```rust
// Host a single activation directly - NO DynamicHub needed
let solar = Arc::new(Solar::new());
TransportServer::builder(solar, converter)
    .with_websocket(8888)
    .serve().await?;

// Calls: solar.observe, solar.mercury.info, solar.earth.luna.info
```

This is the **primary pattern**. Most services should be hosted this way.

#### Multiple Activations (Use DynamicHub)

```rust
// Only use DynamicHub when composing multiple top-level activations
let hub = Arc::new(
    DynamicHub::new("substrate")  // Explicit namespace for your app
        .register(Solar::new())
        .register(Echo::new())
        .register(JsExec::new())
);

TransportServer::builder(hub, converter)
    .with_websocket(8888)
    .serve().await?;

// Calls: substrate.call solar.observe, substrate.call echo.echo
// Or with hub-transport direct routing: solar.observe, echo.echo
```

#### When to Use DynamicHub

**Use DynamicHub when:**
- Hosting multiple top-level activations (like substrate does)
- Need runtime registration of activations
- Building a multi-service application server

**Don't use DynamicHub when:**
- Hosting a single service/plugin
- Your activation already routes to children (Solar, custom hubs)
- Want simpler deployment

**Examples:**
- ✅ **substrate** - Multiple activations (arbor, cone, echo, etc.) → Use `DynamicHub::new("substrate")`
- ❌ **JsExec service** - Single activation → Host `JsExec` directly
- ❌ **Solar explorer** - Solar already routes to planets → Host `Solar` directly
- ✅ **Multi-tenant platform** - Many independent services → Use `DynamicHub::new("platform")`

### 3. Clarify Terminology and Architecture

**Current (confusing):**
- "Plexus" - The framework/hub/router (unclear what it is)
- "Activation" - A plugin (suggests subsidiary to Plexus)
- Impression: Plexus is required infrastructure

**Proposed (clear):**
- **"Activation"** - Anything implementing the trait (primary concept)
  - Can be hosted directly with any transport
  - Can route to children (hub pattern)
  - No framework required
- **"DynamicHub"** - An activation that provides `.register()` (composition tool)
  - Just another activation, not special infrastructure
  - Optional - only needed for composing multiple top-level activations
- **"Hub activation"** - Any activation that routes to children (pattern)
  - Solar routes to planets (hardcoded children)
  - DynamicHub routes to registered activations (dynamic children)
  - Both are just activations implementing ChildRouter

**Key Principle: Activation-First Architecture**

```
┌─────────────────────────────────────────────┐
│ Activation (trait)                          │
│ - The core abstraction                      │
│ - Can be hosted directly                    │
│ - Can route to children (optional)          │
└─────────────────────────────────────────────┘
             │
    ┌────────┴────────┬──────────────┬─────────────┐
    │                 │              │             │
┌───▼────┐      ┌────▼─────┐   ┌────▼────┐   ┌────▼────────┐
│ Echo   │      │  JsExec  │   │  Solar  │   │ DynamicHub  │
│ (leaf) │      │  (leaf)  │   │  (hub)  │   │    (hub)    │
└────────┘      └──────────┘   └─────────┘   └─────────────┘
                                     │              │
                               Hardcoded      Dynamic
                               children       .register()
```

All are equal. None are "special infrastructure."

### 4. Require Explicit Namespace for DynamicHub

**Why this matters:**

Single activations have **fixed namespaces** defined in their implementation:
```rust
#[hub_methods(namespace = "echo", ...)]
impl Echo { ... }
// Always "echo.echo", never changes
```

DynamicHub is a **composition tool** - its namespace should reflect the **application**:
```rust
// substrate server
DynamicHub::new("substrate").register(...)
// Methods: substrate.call, substrate.schema

// Your app
DynamicHub::new("myapp").register(...)
// Methods: myapp.call, myapp.schema
```

Defaulting to "plexus" was wrong - it made it seem like a framework name.

### 5. Implementation: Require Explicit Namespace

**Final implementation:**

```rust
impl DynamicHub {
    /// Create a new DynamicHub with explicit namespace
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(DynamicHubInner {
                namespace: namespace.into(),
                // ...
            }),
        }
    }

    #[deprecated(since = "0.3.0", note = "Use new(namespace) instead")]
    pub fn with_namespace(namespace: impl Into<String>) -> Self {
        Self::new(namespace)
    }
}

// No Default impl - forces explicit namespace choice
```

**Usage:**
```rust
// substrate server
DynamicHub::new("substrate")
    .register(arbor)
    .register(cone)
    .register(echo);

// Generic application
DynamicHub::new("myapp")
    .register(solar)
    .register(jsexec);
```

## Migration Path

### Phase 1: Add Alias (Non-Breaking)

```rust
// hub-core/src/lib.rs
pub use dynamic_hub::DynamicHub;

/// Deprecated: Use DynamicHub instead
#[deprecated(since = "0.3.0", note = "Use DynamicHub instead")]
pub type Plexus = DynamicHub;
```

**Impact:** Zero - existing code continues to work with deprecation warnings.

### Phase 2: Update Internal Usage

```rust
// Update hub-core internals to use DynamicHub
// Update substrate to use DynamicHub
// Update tests to use DynamicHub
```

**Impact:** Internal only - API unchanged.

### Phase 3: Update Public API (Breaking)

```rust
// Remove Plexus type alias
// All references become DynamicHub
```

**Impact:** Breaking change - requires user code updates.

### Phase 4: Rename Module (Breaking)

```rust
// Before
hub-core/src/plexus/

// After
hub-core/src/dynamic_hub/
// OR
hub-core/src/hub/  // If we also rename the pattern
```

**Impact:** Breaking change - imports change.

## Detailed Impact Analysis

### Files to Rename/Update

**hub-core:**
- `src/plexus/plexus.rs` → `src/dynamic_hub/dynamic_hub.rs` (or keep filename)
- `src/plexus/` → `src/dynamic_hub/` (module rename)
- All `pub struct Plexus` → `pub struct DynamicHub`
- All `impl Plexus` → `impl DynamicHub`
- `Plexus::new()` → `DynamicHub::new()` or `DynamicHub::with_namespace()`

**substrate:**
- `src/builder.rs`: `build_plexus()` → `build_hub()` or `build_dynamic_hub()`
- `src/main.rs`: `let plexus = build_plexus()` → `let hub = build_dynamic_hub()`
- `src/lib.rs`: Re-exports
- All activation files that reference `Plexus` type

**hub-transport:**
- Documentation examples that mention Plexus
- No code changes (already generic over `Activation`)

**hub-macro:**
- Possibly references to Plexus in generated code
- May need to update error messages

### What Doesn't Change

✅ **The `Activation` trait** - Stays exactly the same
✅ **hub-transport** - Already generic, no code changes
✅ **Individual activations** - Echo, JsExec, etc. unchanged
✅ **The routing pattern** - ChildRouter trait unchanged
✅ **Arc/Weak pattern** - Weak<DynamicHub> works same as Weak<Plexus>

### Breaking Changes

1. **Type name**: `Plexus` → `DynamicHub`
2. **Import path**: `use hub_core::plexus::Plexus` → `use hub_core::dynamic_hub::DynamicHub`
3. **Function names**: `build_plexus()` → `build_dynamic_hub()`
4. **Variable names**: `plexus: Arc<Plexus>` → `hub: Arc<DynamicHub>`

### Non-Breaking Changes (With Deprecation)

During transition period:
```rust
#[deprecated(note = "Use DynamicHub")]
pub type Plexus = DynamicHub;

#[deprecated(note = "Use build_dynamic_hub")]
pub fn build_plexus() -> Arc<DynamicHub> {
    build_dynamic_hub()
}
```

## Benefits of This Change

### 1. Architectural Clarity

**Before:**
> "Plexus is the central routing layer... wait, but Solar also routes... so is Solar using Plexus? No? Then what IS Plexus? Do I need Plexus to host an activation?"

**After:**
> "Any Activation can be hosted directly. Any activation can route to children if it implements ChildRouter. DynamicHub is just an activation that lets you register children at runtime - it's optional."

### 2. Simpler Mental Model

**Before:** Three concepts (confusing hierarchy)
- Plexus (special framework - required?)
- Activation (plugin interface - lives inside Plexus?)
- Hub (routing pattern - what's the difference from Plexus?)

**After:** Two concepts (flat architecture)
- **Activation** (trait - primary concept)
  - Can be hosted directly
  - Can route to children (optional)
- **DynamicHub** (activation with `.register()`)
  - Optional composition tool
  - Just another activation

### 3. Direct Hosting Pattern Emphasized

**Before:**
```rust
// Users thought they needed DynamicHub for everything
let hub = DynamicHub::new()  // Namespace unclear
    .register(Solar::new());  // Why wrap Solar?
```

**After:**
```rust
// Primary pattern: host activation directly
let solar = Arc::new(Solar::new());
TransportServer::builder(solar, ...).serve().await?;

// DynamicHub only when needed
let hub = DynamicHub::new("substrate")  // Explicit app name
    .register(arbor)
    .register(cone)
    .register(echo);
```

### 3. Direct Hosting Pattern

**Before:**
```rust
// Even for one activation, you think you need Plexus
let plexus = Plexus::new().register(Solar::new());
TransportServer::builder(plexus, ...).serve().await?;
```

**After:**
```rust
// Direct hosting is the primary pattern
let solar = Arc::new(Solar::new());
TransportServer::builder(solar, ...).serve().await?;

// DynamicHub is for composition
let hub = DynamicHub::with_namespace("myapp")
    .register(solar).register(echo);
```

### 4. Better Documentation Flow

**Current flow:**
1. Here's Plexus (framework)
2. Here's how to add activations to Plexus
3. BTW you can nest hubs

**Better flow:**
1. Here's Activation trait (core concept)
2. Any activation can be hosted directly
3. Some activations route to children (hub pattern)
4. DynamicHub helps compose multiple activations
5. You can nest arbitrarily

### 5. Namespace Flexibility

**Before:**
```rust
Plexus::new()  // Always defaults to "plexus" namespace
```

**After:**
```rust
DynamicHub::with_namespace("myapp")  // Explicit namespace choice
```

Users can choose meaningful namespaces:
- `"substrate"` for substrate server
- `"lforge"` for hyperforge
- `"control"` for control flow orchestrator

## Risks and Mitigations

### Risk: Breaking Change Disruption

**Mitigation:**
- Phase 1: Deprecation alias (non-breaking)
- Wait 1-2 versions before removing
- Provide migration guide
- Update all first-party code first

### Risk: Confusion During Transition

**Mitigation:**
- Clear deprecation messages
- Migration guide in changelog
- Both names documented temporarily
- FAQ: "What happened to Plexus?"

### Risk: External Dependencies

**Mitigation:**
- Search for external uses before Phase 3
- Coordinate with known users
- Provide migration timeline
- Keep alias for 2+ versions

### Risk: Lost "Brand Recognition"

**Mitigation:**
- "Plexus" wasn't a strong brand (internal tool)
- "DynamicHub" is more descriptive
- Documentation can mention "formerly Plexus"

## Implementation Checklist

### Phase 1: Alias and Deprecation ✅ COMPLETED

- [x] Add `pub type Plexus = DynamicHub` with deprecation
- [x] Rename struct Plexus to DynamicHub
- [x] Update all impl blocks to use DynamicHub
- [x] **Require explicit namespace**: Remove Default, make new() take namespace param
- [x] Deprecate with_namespace() in favor of new(namespace)
- [x] Update hub-core documentation (README, lib.rs)
- [x] Add migration guide to CHANGELOG
- [x] Update all hub-core tests to use DynamicHub::new("test")
- [x] Test that existing code still compiles with deprecation warnings

### Phase 2: Internal Migration ✅ COMPLETED

- [x] Update hub-core internal usage (builder.rs)
- [x] Update substrate to use DynamicHub::new("substrate")
- [x] Update substrate tests to use DynamicHub
- [x] Update substrate examples to use DynamicHub
- [x] Update hub-transport docs to mention DynamicHub and direct hosting
- [x] Add architecture documentation about when to use DynamicHub vs direct hosting

### Phase 3: Remove Alias (FUTURE - Breaking)

- [ ] Remove `type Plexus = DynamicHub`
- [ ] Remove deprecated with_namespace() function
- [ ] Update version (0.3.0 → 0.4.0)
- [ ] Announce breaking change

### Phase 4: Module Rename (OPTIONAL - Future)

- [ ] Rename `src/plexus/` directory to `src/dynamic_hub/`
- [ ] Update all import paths
- [ ] Update version (0.4.0 → 0.5.0)
- [ ] Update documentation

**Note**: Phases 3 and 4 are future work. Current implementation (Phases 1-2) maintains backwards compatibility via deprecated type alias.

## Alternative Approaches

### Alternative 1: Keep "Plexus" Name

**Pros:**
- No breaking changes
- Existing code continues to work
- No migration needed

**Cons:**
- Doesn't fix the confusion
- Perpetuates architectural misunderstanding
- Namespace collision remains

**Decision:** Reject - doesn't solve the problem.

### Alternative 2: Rename to "Hub"

```rust
pub struct Hub { ... }
```

**Pros:**
- Simple, clear name
- Matches the pattern

**Cons:**
- Too generic - what if users want their own Hub type?
- Doesn't distinguish from hub pattern
- Less clear that it's for dynamic registration

**Decision:** Reject - "DynamicHub" is more specific.

### Alternative 3: Rename to "Registry"

```rust
pub struct ActivationRegistry { ... }
```

**Pros:**
- Emphasizes the registration aspect
- Clear purpose

**Cons:**
- Loses the "hub" routing concept
- Sounds like a lookup table, not an activation
- Doesn't convey that it implements Activation

**Decision:** Reject - doesn't capture full purpose.

### Alternative 4: Rename to "Composer"

```rust
pub struct ActivationComposer { ... }
```

**Pros:**
- Emphasizes composition
- Modern pattern name

**Cons:**
- Less intuitive for routing concept
- "Compose" suggests transformation, not routing

**Decision:** Reject - routing is the key feature.

## Related Patterns

### Composite Pattern

DynamicHub implements the Composite pattern from GoF:
- Component: `Activation` trait
- Leaf: `Echo`, `JsExec` (no children)
- Composite: `DynamicHub`, `Solar` (have children)

### Router Pattern

DynamicHub implements a router:
- Routes based on namespace prefix
- Forwards to appropriate handler
- Supports nested routing

### Registry Pattern

DynamicHub also provides registration:
- `.register()` adds components
- HashMap lookup for routing
- Dynamic composition at runtime

## Future Extensions

### 1. Static Hub Macro

```rust
#[hub(namespace = "myapp")]
struct MyHub {
    solar: Solar,
    echo: Echo,
    jsexec: JsExec,
}

// Generates Activation impl with hardcoded routing
```

This would be like Solar (hardcoded children) but for arbitrary activations.

### 2. Conditional Registration

```rust
DynamicHub::with_namespace("app")
    .register(solar)
    .register_if(feature_enabled, jsexec)
    .register_lazy("arbor", || Arbor::new(config));
```

### 3. Activation Groups

```rust
DynamicHub::with_namespace("app")
    .register_group("tools", [jsexec, bash])
    .register_group("ai", [cone, arbor]);

// Calls: app.tools.jsexec.execute, app.ai.cone.create
```

### 4. Dynamic Unregistration

```rust
hub.unregister("echo");  // Remove activation at runtime
```

For hot-reloading or conditional activation.

## Documentation Updates

### hub-core README

**Before:**
> Plexus is the central routing layer for activations...

**After:**
> DynamicHub is an activation that dynamically routes to registered child activations. Any activation can be a hub - Solar routes to planets, DynamicHub routes to registered activations.

### Getting Started Guide

**New structure:**
1. **Activation Trait** - Core concept
2. **Hosting a Single Activation** - Direct pattern
3. **Hub Activations** - Routing pattern (Solar example)
4. **DynamicHub** - Composition tool for multiple activations
5. **Nested Hubs** - Solar within DynamicHub

### API Documentation

```rust
/// DynamicHub - An activation that routes to dynamically registered children
///
/// Unlike hub activations with hardcoded children (like Solar),
/// DynamicHub allows registering activations at runtime via `.register()`.
///
/// # Direct Hosting
///
/// For a single activation, host it directly:
/// ```
/// let solar = Arc::new(Solar::new());
/// TransportServer::builder(solar, converter).serve().await?;
/// ```
///
/// # Composition
///
/// For multiple top-level activations, use DynamicHub:
/// ```
/// let hub = DynamicHub::with_namespace("myapp")
///     .register(Solar::new())
///     .register(Echo::new());
/// ```
pub struct DynamicHub { ... }
```

## Success Metrics

**After migration:**

1. ✅ New users understand they can host activations directly
2. ✅ "Hub" means routing pattern, not framework requirement
3. ✅ DynamicHub is seen as composition tool, not core infrastructure
4. ✅ Examples lead with direct hosting
5. ✅ No namespace confusion (type ≠ default namespace)
6. ✅ Architecture diagrams show Activation as primary concept

## Conclusion

Renaming `Plexus` to `DynamicHub` and requiring explicit namespaces achieves:

1. **Clarifies architecture** - Activation is primary, DynamicHub is optional
2. **Reduces confusion** - Clear that DynamicHub is not required infrastructure
3. **Better naming** - Type describes what it does (dynamic hub with .register())
4. **Simpler onboarding** - Start with direct activation hosting
5. **Preserves power** - Composition still available when needed
6. **Intentional naming** - Explicit namespace forces users to name their application

The changes are breaking but justified by the architectural clarity they provide.

### What Was Implemented (Phases 1-2)

✅ **Phase 1 Complete:**
- Struct renamed to DynamicHub
- Deprecated type alias `Plexus` maintains backwards compatibility
- Explicit namespace required: `DynamicHub::new(namespace)`
- Updated all documentation and examples

✅ **Phase 2 Complete:**
- substrate uses `DynamicHub::new("substrate")`
- All tests updated
- README emphasizes direct hosting pattern
- Architecture docs explain when to use DynamicHub vs direct hosting

### Migration Status

**Current state:** Backwards compatible with deprecation warnings
- Old code using `Plexus::new()` will show deprecation warnings
- Type alias `Plexus` still works
- Migration path is clear via deprecation messages and documentation

**Future phases (not yet scheduled):**
- Phase 3: Remove Plexus type alias (breaking change)
- Phase 4: Rename module directory (optional)

### Key Takeaway

**Any activation can be hosted directly.** DynamicHub is just a composition tool for when you need multiple top-level activations. This is the fundamental insight that drives all the naming and documentation changes.
