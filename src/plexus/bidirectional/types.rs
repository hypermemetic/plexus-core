//! Bidirectional streaming types
//!
//! This module defines the core types for bidirectional communication in Plexus RPC.
//! These types enable server-to-client requests during streaming execution, supporting
//! interactive workflows like confirmations, prompts, and selection menus.
//!
//! # Type System
//!
//! The bidirectional system uses a **generic type design** that separates:
//!
//! 1. **Standard types** ([`StandardRequest`]/[`StandardResponse`]) for common UI patterns
//! 2. **Custom types** that activations can define for domain-specific interactions
//!
//! # Wire Format
//!
//! All types use `serde` for serialization. The standard types use internally-tagged
//! enums for JSON-friendly wire format:
//!
//! ```json
//! // StandardRequest::Confirm
//! { "type": "confirm", "message": "Delete file?", "default": false }
//!
//! // StandardResponse::Confirmed
//! { "confirmed": true }
//! ```
//!
//! # Error Handling
//!
//! [`BidirError`] covers all failure modes:
//! - Transport doesn't support bidirectional ([`BidirError::NotSupported`])
//! - User cancelled ([`BidirError::Cancelled`])
//! - Request timed out ([`BidirError::Timeout`])
//! - Type/serialization errors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Error types for bidirectional communication
///
/// This enum covers all failure modes that can occur during bidirectional
/// request/response cycles. Activations should handle these errors gracefully,
/// especially [`BidirError::NotSupported`] which indicates the transport
/// cannot support interactive features.
///
/// # Common Patterns
///
/// ```rust,ignore
/// use plexus_core::plexus::bidirectional::{BidirError, StandardBidirChannel};
///
/// async fn my_method(ctx: &StandardBidirChannel) {
///     match ctx.confirm("Proceed?").await {
///         Ok(true) => { /* user confirmed */ }
///         Ok(false) => { /* user declined */ }
///         Err(BidirError::NotSupported) => {
///             // Non-interactive transport - use safe defaults
///         }
///         Err(BidirError::Cancelled) => {
///             // User explicitly cancelled
///         }
///         Err(BidirError::Timeout(_)) => {
///             // User didn't respond in time
///         }
///         Err(e) => {
///             // Other errors - log and handle
///             eprintln!("Bidirectional error: {}", e);
///         }
///     }
/// }
/// ```
#[derive(Debug, Clone, thiserror::Error)]
pub enum BidirError {
    /// Transport does not support bidirectional communication.
    ///
    /// This is a normal condition - many transports (HTTP, some MCP configs)
    /// cannot support server-to-client requests. Activations should have
    /// fallback behavior when this error occurs.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// match ctx.confirm("Delete?").await {
    ///     Err(BidirError::NotSupported) => {
    ///         // Don't delete without confirmation
    ///         return Err("Interactive confirmation required");
    ///     }
    ///     // ...
    /// }
    /// ```
    #[error("Bidirectional communication not supported by this transport")]
    NotSupported,

    /// Request timed out waiting for client response.
    ///
    /// The timeout value (in milliseconds) is included. Default timeout
    /// is 30 seconds, configurable via `request_with_timeout()` method.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// match ctx.confirm("Confirm?").await {
    ///     Err(BidirError::Timeout(ms)) => {
    ///         println!("No response after {}ms", ms);
    ///     }
    ///     // ...
    /// }
    /// ```
    #[error("Request timed out after {0}ms")]
    Timeout(u64),

    /// Client explicitly cancelled the request.
    ///
    /// This indicates the user chose to cancel rather than respond.
    /// Different from declining - cancel means "abort the workflow".
    #[error("Request was cancelled by client")]
    Cancelled,

    /// Response type doesn't match expected type.
    ///
    /// This usually indicates a bug in client code or a protocol mismatch.
    /// For example, responding with `Text` to a `Confirm` request.
    #[error("Type mismatch: expected {expected}, got {got}")]
    TypeMismatch {
        /// Expected type name
        expected: String,
        /// Actual type received
        got: String,
    },

    /// Failed to serialize or deserialize request/response.
    ///
    /// Contains the underlying serialization error message.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Transport-level error during communication.
    ///
    /// This covers network errors, connection drops, etc.
    #[error("Transport error: {0}")]
    Transport(String),

    /// Unknown request ID (response for non-existent request).
    ///
    /// This can happen if:
    /// - The request already timed out
    /// - The request was cancelled
    /// - The request ID was corrupted
    #[error("Unknown request ID")]
    UnknownRequest,

    /// Response channel was closed before response received.
    ///
    /// This typically means the waiting task was cancelled or dropped.
    #[error("Response channel closed")]
    ChannelClosed,
}

/// Standard request types for common interactive UI patterns.
///
/// These request types cover the most common server-to-client interactions:
///
/// - **Confirm**: Yes/no questions before important actions
/// - **Prompt**: Free-form text input from the user
/// - **Select**: Choose one or more options from a list
///
/// For domain-specific interactions (e.g., image quality selection, custom
/// dialogs), define your own request/response enums and use
/// [`BidirChannel<YourRequest, YourResponse>`](super::BidirChannel).
///
/// # Wire Format
///
/// Uses internally-tagged JSON (`#[serde(tag = "type")]`):
///
/// ```json
/// // Confirm request
/// {
///   "type": "confirm",
///   "message": "Delete 3 files?",
///   "default": false
/// }
///
/// // Prompt request
/// {
///   "type": "prompt",
///   "message": "Enter project name:",
///   "default": "my-project",
///   "placeholder": "project-name"
/// }
///
/// // Select request
/// {
///   "type": "select",
///   "message": "Choose template:",
///   "options": [
///     { "value": "minimal", "label": "Minimal", "description": "Bare-bones starter" },
///     { "value": "full", "label": "Full Featured" }
///   ],
///   "multiSelect": false
/// }
/// ```
///
/// # Client Implementation
///
/// Clients should display appropriate UI for each request type:
///
/// | Type | UI Suggestion |
/// |------|---------------|
/// | `confirm` | Yes/No buttons or checkbox |
/// | `prompt` | Text input field |
/// | `select` | Dropdown, radio buttons, or checkbox list |
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StandardRequest {
    /// Binary yes/no confirmation request.
    ///
    /// Use this for important decisions like:
    /// - Confirming destructive operations ("Delete 3 files?")
    /// - Proceeding with potentially expensive operations
    /// - Accepting terms or conditions
    ///
    /// The `default` field suggests the default choice if the user
    /// doesn't explicitly respond (e.g., just presses Enter).
    Confirm {
        /// Question to ask the user.
        /// Should be a clear yes/no question.
        message: String,

        /// Default answer if user accepts without explicit choice.
        /// - `Some(true)` = default to "yes"
        /// - `Some(false)` = default to "no"
        /// - `None` = require explicit choice
        #[serde(skip_serializing_if = "Option::is_none")]
        default: Option<bool>,
    },

    /// Free-form text input request.
    ///
    /// Use this for collecting:
    /// - Names, titles, identifiers
    /// - Paths, URLs
    /// - Custom values not in a predefined list
    ///
    /// For password/sensitive input, clients should use appropriate
    /// input masking.
    Prompt {
        /// Prompt message shown to the user.
        message: String,

        /// Default value to pre-fill in the input.
        /// User can accept or modify.
        #[serde(skip_serializing_if = "Option::is_none")]
        default: Option<String>,

        /// Placeholder text shown when input is empty.
        /// Provides a hint about expected format.
        #[serde(skip_serializing_if = "Option::is_none")]
        placeholder: Option<String>,
    },

    /// Selection request for choosing from options.
    ///
    /// Use this when the valid choices are known ahead of time.
    /// Supports both single and multiple selection.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let options = vec![
    ///     SelectOption::new("dev", "Development")
    ///         .with_description("Local dev environment"),
    ///     SelectOption::new("prod", "Production")
    ///         .with_description("Live servers"),
    /// ];
    /// let selected = ctx.select("Choose environment:", options).await?;
    /// ```
    Select {
        /// Selection prompt shown to the user.
        message: String,

        /// Available options to choose from.
        /// Each option has a value (returned) and label (displayed).
        options: Vec<SelectOption>,

        /// Whether to allow selecting multiple options.
        /// - `false` (default): single selection, returns one value
        /// - `true`: multiple selection, returns zero or more values
        #[serde(default)]
        multi_select: bool,
    },
}

/// Standard response types matching [`StandardRequest`].
///
/// Each variant corresponds to a request type:
///
/// | Request | Response |
/// |---------|----------|
/// | `Confirm` | `Confirmed(bool)` |
/// | `Prompt` | `Text(String)` |
/// | `Select` | `Selected(Vec<String>)` |
/// | Any | `Cancelled` (user cancelled) |
///
/// # Wire Format
///
/// Uses externally-tagged JSON (Rust enum default):
///
/// ```json
/// // Confirmed response
/// { "confirmed": true }
///
/// // Text response
/// { "text": "user-input-here" }
///
/// // Selected response (single selection)
/// { "selected": ["dev"] }
///
/// // Selected response (multi-selection)
/// { "selected": ["option1", "option2"] }
///
/// // Cancelled response
/// { "cancelled": null }
/// ```
///
/// **Note for TypeScript clients**: The generated types use a tagged format:
/// ```typescript
/// // TypeScript format
/// { type: 'confirmed', value: true }
/// { type: 'text', value: 'user-input' }
/// { type: 'selected', values: ['dev'] }
/// { type: 'cancelled' }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StandardResponse {
    /// User confirmed (true) or declined (false).
    ///
    /// Response to `StandardRequest::Confirm`.
    /// - `Confirmed(true)` = user said yes
    /// - `Confirmed(false)` = user said no
    Confirmed(bool),

    /// User entered text.
    ///
    /// Response to `StandardRequest::Prompt`.
    /// May be empty string if user submitted without entering text.
    Text(String),

    /// User selected one or more options (by value).
    ///
    /// Response to `StandardRequest::Select`.
    /// Contains the `value` field(s) from selected [`SelectOption`]s.
    ///
    /// - For single-select: vector with exactly one element
    /// - For multi-select: vector with zero or more elements
    Selected(Vec<String>),

    /// User cancelled the request.
    ///
    /// Can be sent in response to any request type.
    /// Indicates the user chose to abort rather than respond.
    /// This is different from declining (Confirmed(false)) - cancel
    /// means "don't proceed with the workflow at all".
    Cancelled,
}

/// An option in a [`StandardRequest::Select`] request.
///
/// Each option has:
/// - **value**: Machine-readable identifier returned in the response
/// - **label**: Human-readable text displayed to the user
/// - **description**: Optional additional context about the option
///
/// # Wire Format
///
/// ```json
/// {
///   "value": "prod",
///   "label": "Production",
///   "description": "Live environment - requires approval"
/// }
/// ```
///
/// # Example
///
/// ```rust
/// use plexus_core::plexus::bidirectional::SelectOption;
///
/// let options = vec![
///     SelectOption::new("minimal", "Minimal Starter")
///         .with_description("Basic project structure"),
///     SelectOption::new("full", "Full Featured")
///         .with_description("All features included"),
///     SelectOption::new("api", "API Only"),  // No description
/// ];
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SelectOption {
    /// Machine-readable value returned when this option is selected.
    ///
    /// This is what appears in `StandardResponse::Selected`.
    /// Should be a stable identifier (e.g., "dev", "prod", "option_1").
    pub value: String,

    /// Human-readable label displayed to the user.
    ///
    /// Should be concise but descriptive (e.g., "Development", "Production").
    pub label: String,

    /// Optional description providing additional context.
    ///
    /// Use for longer explanations that don't fit in the label.
    /// Clients may display this as a tooltip or secondary text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl SelectOption {
    /// Create a new select option with value and label.
    ///
    /// # Arguments
    ///
    /// * `value` - Machine-readable identifier (returned in response)
    /// * `label` - Human-readable display text
    ///
    /// # Example
    ///
    /// ```rust
    /// use plexus_core::plexus::bidirectional::SelectOption;
    ///
    /// let opt = SelectOption::new("dev", "Development Environment");
    /// assert_eq!(opt.value, "dev");
    /// assert_eq!(opt.label, "Development Environment");
    /// ```
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            description: None,
        }
    }

    /// Add a description to this option.
    ///
    /// # Example
    ///
    /// ```rust
    /// use plexus_core::plexus::bidirectional::SelectOption;
    ///
    /// let opt = SelectOption::new("prod", "Production")
    ///     .with_description("Live environment - changes affect real users");
    ///
    /// assert_eq!(opt.description, Some("Live environment - changes affect real users".into()));
    /// ```
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_request_serialization() {
        let req = StandardRequest::Confirm {
            message: "Continue?".into(),
            default: Some(false),
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["type"], "confirm");
        assert_eq!(json["message"], "Continue?");
        assert_eq!(json["default"], false);
    }

    #[test]
    fn test_standard_response_serialization() {
        let resp = StandardResponse::Confirmed(true);

        let json = serde_json::to_value(&resp).unwrap();
        // Externally tagged: { "confirmed": true }
        assert_eq!(json["confirmed"], true);

        // Test round-trip
        let roundtrip: StandardResponse = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, StandardResponse::Confirmed(true));
    }

    #[test]
    fn test_select_option_builder() {
        let opt = SelectOption::new("prod", "Production")
            .with_description("Requires approval");

        assert_eq!(opt.value, "prod");
        assert_eq!(opt.label, "Production");
        assert_eq!(opt.description, Some("Requires approval".into()));
    }

    #[test]
    fn test_bidir_error_display() {
        let err = BidirError::Timeout(30000);
        assert_eq!(err.to_string(), "Request timed out after 30000ms");

        let err = BidirError::TypeMismatch {
            expected: "Confirmed".into(),
            got: "Text".into(),
        };
        assert_eq!(
            err.to_string(),
            "Type mismatch: expected Confirmed, got Text"
        );
    }
}
