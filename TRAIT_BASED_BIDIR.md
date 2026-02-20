# Trait-Based Bidirectional Protocol

This document describes the new trait-based bidirectional protocol that replaces the enum-based system.

## Overview

The trait-based protocol eliminates the need for enum wrapping and allows users to work directly with typed structs. This provides better ergonomics and type safety.

## Key Changes

### Before (Enum-Based)

```rust
use plexus_core::plexus::bidirectional::{StandardRequest, StandardResponse, StandardBidirChannel};

async fn old_style(ctx: &StandardBidirChannel) -> Result<(), BidirError> {
    // Had to wrap in enum variants
    let response = ctx.request(StandardRequest::Confirm {
        message: "Delete file?".into(),
        default: None,
    }).await?;

    // Had to pattern match on response
    match response {
        StandardResponse::Confirmed { value } => {
            if value {
                println!("User confirmed");
            }
        }
        StandardResponse::Cancelled => {
            println!("User cancelled");
        }
        _ => {
            return Err(BidirError::TypeMismatch {
                expected: "Confirmed".into(),
                got: format!("{:?}", response),
            });
        }
    }

    Ok(())
}
```

### After (Trait-Based)

```rust
use plexus_core::plexus::bidirectional::{
    MultiBidirChannel, ConfirmRequest, ConfirmedResponse, BidirError
};

async fn new_style(ctx: &MultiBidirChannel) -> Result<(), BidirError> {
    // Work directly with typed structs
    let request = ConfirmRequest {
        message: "Delete file?".into(),
        default: None,
    };

    let response: ConfirmedResponse = ctx.request(request).await?;

    // Direct field access - no pattern matching needed
    if response.value {
        println!("User confirmed");
    }

    Ok(())
}
```

## Core Types

### Traits

- `BidirRequest` - Trait for all request types
- `BidirResponse` - Trait for all response types

### Well-Known Request Types

- `ConfirmRequest` - Yes/no confirmation
- `PromptRequest` - Text input
- `SelectRequest` - Selection from options

### Well-Known Response Types

- `ConfirmedResponse` - Boolean confirmation result
- `TextResponse` - Text input result
- `SelectedResponse` - Selection result (Vec<String>)
- `CancelledResponse` - User cancellation

### Channel Types

- `MultiBidirChannel` - Generic channel that accepts any BidirRequest/BidirResponse types
- `BidirChannel<Req, Resp>` - Type-specific channel (still available)

## Usage Examples

### 1. Using Convenience Methods

The easiest way to use the new system:

```rust
use plexus_core::plexus::bidirectional::MultiBidirChannel;

async fn wizard(ctx: &MultiBidirChannel) -> Result<(), BidirError> {
    // Yes/no confirmation
    if ctx.confirm("Continue?").await? {
        println!("Continuing...");
    }

    // Text input
    let name = ctx.prompt("Enter your name:").await?;
    println!("Hello, {}!", name);

    // Selection
    let options = vec![
        SelectOption::new("dev", "Development"),
        SelectOption::new("prod", "Production"),
    ];
    let selected = ctx.select("Choose environment:", options).await?;
    println!("Selected: {:?}", selected);

    Ok(())
}
```

### 2. Using Typed Requests/Responses

For more control:

```rust
use plexus_core::plexus::bidirectional::{
    MultiBidirChannel, ConfirmRequest, ConfirmedResponse,
    PromptRequest, TextResponse
};

async fn typed_requests(ctx: &MultiBidirChannel) -> Result<(), BidirError> {
    // Create request
    let request = ConfirmRequest {
        message: "Delete 5 files?".into(),
        default: Some(false),  // Default to "no"
    };

    // Get typed response
    let response: ConfirmedResponse = ctx.request(request).await?;

    if response.value {
        // Prompt for confirmation phrase
        let prompt = PromptRequest {
            message: "Type 'DELETE' to confirm:".into(),
            default: None,
            placeholder: Some("DELETE".into()),
        };

        let confirmation: TextResponse = ctx.request(prompt).await?;

        if confirmation.value == "DELETE" {
            println!("Deleting files...");
        }
    }

    Ok(())
}
```

### 3. Custom Request/Response Types

Implement the traits for domain-specific types:

```rust
use plexus_core::plexus::bidirectional::protocol::{BidirRequest, BidirResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageQualityRequest {
    pub current_quality: u8,
    pub min: u8,
    pub max: u8,
}

impl BidirRequest for ImageQualityRequest {
    fn type_tag(&self) -> &'static str {
        "image_quality"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageQualityResponse {
    pub quality: u8,
}

impl BidirResponse for ImageQualityResponse {
    fn type_tag(&self) -> &'static str {
        "image_quality_response"
    }
}

async fn custom_interaction(ctx: &MultiBidirChannel) -> Result<(), BidirError> {
    let request = ImageQualityRequest {
        current_quality: 85,
        min: 50,
        max: 100,
    };

    let response: ImageQualityResponse = ctx.request(request).await?;

    println!("User selected quality: {}", response.quality);

    Ok(())
}
```

## Migration Guide

### Step 1: Update Channel Type

Replace `StandardBidirChannel` with `MultiBidirChannel`:

```rust
// Before
async fn my_method(ctx: &StandardBidirChannel) { ... }

// After
async fn my_method(ctx: &MultiBidirChannel) { ... }
```

### Step 2: Update Request Construction

Replace enum variants with struct construction:

```rust
// Before
StandardRequest::Confirm {
    message: "Delete?".into(),
    default: None,
}

// After
ConfirmRequest {
    message: "Delete?".into(),
    default: None,
}
```

### Step 3: Update Response Handling

Replace pattern matching with direct field access:

```rust
// Before
match response {
    StandardResponse::Confirmed { value } => {
        if value { ... }
    }
    _ => { ... }
}

// After
let response: ConfirmedResponse = ctx.request(request).await?;
if response.value { ... }
```

## Backwards Compatibility

The old enum-based types are still available but deprecated:

- `StandardBidirChannel` (use `MultiBidirChannel`)
- `StandardRequest` (use `ConfirmRequest`, `PromptRequest`, etc.)
- `StandardResponse` (use `ConfirmedResponse`, `TextResponse`, etc.)

Existing code will continue to work with deprecation warnings.

## Benefits

1. **No forced enum matching** - Only handle the types you actually use
2. **Better type safety** - Compiler ensures request/response types match
3. **Cleaner code** - Direct struct usage instead of enum wrapping
4. **Extensibility** - Easy to add custom types without modifying core enums
5. **Better IDE support** - Type inference works better with concrete types

## Wire Format

The wire format remains unchanged - all types use JSON with type tags:

```json
{
  "type": "confirm",
  "message": "Delete file?",
  "default": false
}
```

This ensures compatibility with existing clients.
