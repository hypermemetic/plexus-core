//! Bidirectional streaming types
//!
//! This module defines the core types for bidirectional communication in Plexus RPC.
//! These types enable server-to-client requests during streaming execution, supporting
//! interactive workflows like confirmations, prompts, and selection menus.
//!
//! # Type System
//!
//! The bidirectional system now uses a **trait-based design**:
//!
//! 1. **Trait-based protocol** ([`BidirRequest`]/[`BidirResponse`]) - Core traits
//! 2. **Well-known types** - Plain structs for common UI patterns (Confirm, Prompt, Select)
//! 3. **Union types** - Enums for backwards compatibility ([`WellKnownRequest`]/[`WellKnownResponse`])
//! 4. **Legacy types** - Deprecated enum-based types ([`StandardRequest`]/[`StandardResponse`])
//!
//! # Migration Guide
//!
//! **Old (enum-based)**:
//! ```rust,ignore
//! let resp = ctx.request(StandardRequest::Confirm {
//!     message: "Delete?".into(),
//!     default: None,
//! }).await?;
//! ```
//!
//! **New (trait-based)**:
//! ```rust,ignore
//! let resp = ctx.request(ConfirmRequest {
//!     message: "Delete?".into(),
//!     default: None,
//! }).await?;
//! ```
//!
//! # Wire Format
//!
//! All types use `serde` for serialization with internally-tagged JSON:
//!
//! ```json
//! // ConfirmRequest
//! { "type": "confirm", "message": "Delete file?", "default": false }
//!
//! // ConfirmedResponse
//! { "type": "confirmed", "value": true }
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
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::protocol::{BidirRequest, BidirResponse};

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

// =============================================================================
// Well-Known Request Types (Plain Structs)
// =============================================================================

/// Binary yes/no confirmation request.
///
/// Use this for important decisions like:
/// - Confirming destructive operations ("Delete 3 files?")
/// - Proceeding with potentially expensive operations
/// - Accepting terms or conditions
///
/// # Wire Format
///
/// ```json
/// {
///   "type": "confirm",
///   "message": "Delete file?",
///   "default": false
/// }
/// ```
///
/// # Example
///
/// ```rust,ignore
/// use plexus_core::plexus::bidirectional::{ConfirmRequest, ConfirmedResponse};
///
/// let request = ConfirmRequest {
///     message: "Delete this file?".into(),
///     default: Some(false),
/// };
/// let response: ConfirmedResponse = ctx.request(request).await?;
/// if response.value {
///     // User confirmed
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ConfirmRequest {
    /// Question to ask the user.
    /// Should be a clear yes/no question.
    pub message: String,

    /// Default answer if user accepts without explicit choice.
    /// - `Some(true)` = default to "yes"
    /// - `Some(false)` = default to "no"
    /// - `None` = require explicit choice
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
}

impl BidirRequest for ConfirmRequest {
    fn type_tag(&self) -> &'static str {
        "confirm"
    }
}

/// Free-form text input request.
///
/// Use this for collecting:
/// - Names, titles, identifiers
/// - Paths, URLs
/// - Custom values not in a predefined list
///
/// # Wire Format
///
/// ```json
/// {
///   "type": "prompt",
///   "message": "Enter project name:",
///   "default": "my-project",
///   "placeholder": "project-name"
/// }
/// ```
///
/// # Example
///
/// ```rust,ignore
/// use plexus_core::plexus::bidirectional::{PromptRequest, TextResponse};
///
/// let request = PromptRequest {
///     message: "Enter your name:".into(),
///     default: None,
///     placeholder: Some("John Doe".into()),
/// };
/// let response: TextResponse = ctx.request(request).await?;
/// println!("User entered: {}", response.value);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct PromptRequest {
    /// Prompt message shown to the user.
    pub message: String,

    /// Default value to pre-fill in the input.
    /// User can accept or modify.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,

    /// Placeholder text shown when input is empty.
    /// Provides a hint about expected format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

impl BidirRequest for PromptRequest {
    fn type_tag(&self) -> &'static str {
        "prompt"
    }
}

/// Selection request for choosing from options.
///
/// Use this when the valid choices are known ahead of time.
/// Supports both single and multiple selection.
///
/// # Wire Format
///
/// ```json
/// {
///   "type": "select",
///   "message": "Choose environment:",
///   "options": [
///     { "value": "dev", "label": "Development", "description": "Local dev" },
///     { "value": "prod", "label": "Production" }
///   ],
///   "multiSelect": false
/// }
/// ```
///
/// # Example
///
/// ```rust,ignore
/// use plexus_core::plexus::bidirectional::{SelectRequest, SelectOption, SelectedResponse};
///
/// let request = SelectRequest {
///     message: "Choose environment:".into(),
///     options: vec![
///         SelectOption::new("dev", "Development")
///             .with_description("Local development environment"),
///         SelectOption::new("prod", "Production")
///             .with_description("Live environment"),
///     ],
///     multi_select: false,
/// };
/// let response: SelectedResponse = ctx.request(request).await?;
/// println!("Selected: {:?}", response.values);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SelectRequest {
    /// Selection prompt shown to the user.
    pub message: String,

    /// Available options to choose from.
    /// Each option has a value (returned) and label (displayed).
    pub options: Vec<SelectOption>,

    /// Whether to allow selecting multiple options.
    /// - `false` (default): single selection, returns one value
    /// - `true`: multiple selection, returns zero or more values
    #[serde(default, rename = "multiSelect")]
    pub multi_select: bool,
}

impl BidirRequest for SelectRequest {
    fn type_tag(&self) -> &'static str {
        "select"
    }
}

// =============================================================================
// Well-Known Response Types (Plain Structs)
// =============================================================================

/// User confirmed (true) or declined (false).
///
/// Response to [`ConfirmRequest`].
/// - `value: true` = user said yes
/// - `value: false` = user said no
///
/// # Wire Format
///
/// ```json
/// { "type": "confirmed", "value": true }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ConfirmedResponse {
    /// Whether the user confirmed (true) or declined (false)
    pub value: bool,
}

impl BidirResponse for ConfirmedResponse {
    fn type_tag(&self) -> &'static str {
        "confirmed"
    }
}

/// User entered text or provided a value.
///
/// Response to [`PromptRequest`].
/// May be empty if user submitted without entering text.
///
/// # Wire Format
///
/// ```json
/// { "type": "text", "value": "user input" }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct TextResponse {
    /// The value entered or provided by the user
    pub value: String,
}

impl BidirResponse for TextResponse {
    fn type_tag(&self) -> &'static str {
        "text"
    }
}

/// User selected one or more options (by value).
///
/// Response to [`SelectRequest`].
/// Contains the `value` field(s) from selected options.
///
/// - For single-select: vector with exactly one element
/// - For multi-select: vector with zero or more elements
///
/// # Wire Format
///
/// ```json
/// { "type": "selected", "values": ["dev", "staging"] }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SelectedResponse {
    /// The values of selected options
    pub values: Vec<String>,
}

impl BidirResponse for SelectedResponse {
    fn type_tag(&self) -> &'static str {
        "selected"
    }
}

/// User cancelled the request.
///
/// Can be sent in response to any request type.
/// Indicates the user chose to abort rather than respond.
/// This is different from declining (ConfirmedResponse { value: false }) - cancel
/// means "don't proceed with the workflow at all".
///
/// # Wire Format
///
/// ```json
/// { "type": "cancelled" }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct CancelledResponse {}

impl BidirResponse for CancelledResponse {
    fn type_tag(&self) -> &'static str {
        "cancelled"
    }
}

// =============================================================================
// Well-Known Union Types (for convenience)
// =============================================================================

/// Union type of all well-known request types.
///
/// This enum provides a convenient way to work with all well-known request types
/// when you need to handle multiple types in a single context. It's useful for:
/// - Testing with auto-responders
/// - Generic request handlers
/// - Backwards compatibility with enum-based code
///
/// # Example
///
/// ```rust,ignore
/// use plexus_core::plexus::bidirectional::WellKnownRequest;
///
/// let request = WellKnownRequest::Confirm(ConfirmRequest {
///     message: "Continue?".into(),
///     default: None,
/// });
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WellKnownRequest {
    /// Confirmation request
    Confirm(ConfirmRequest),
    /// Text prompt request
    Prompt(PromptRequest),
    /// Selection request
    Select(SelectRequest),
}

impl BidirRequest for WellKnownRequest {
    fn type_tag(&self) -> &'static str {
        match self {
            WellKnownRequest::Confirm(_) => "confirm",
            WellKnownRequest::Prompt(_) => "prompt",
            WellKnownRequest::Select(_) => "select",
        }
    }
}

/// Union type of all well-known response types.
///
/// This enum provides a convenient way to work with all well-known response types
/// when you need to handle multiple types in a single context.
///
/// # Example
///
/// ```rust,ignore
/// use plexus_core::plexus::bidirectional::WellKnownResponse;
///
/// let response = WellKnownResponse::Confirmed(ConfirmedResponse { value: true });
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WellKnownResponse {
    /// Confirmation response
    Confirmed(ConfirmedResponse),
    /// Text input response
    Text(TextResponse),
    /// Selection response
    Selected(SelectedResponse),
    /// Cancellation response
    Cancelled(CancelledResponse),
}

impl BidirResponse for WellKnownResponse {
    fn type_tag(&self) -> &'static str {
        match self {
            WellKnownResponse::Confirmed(_) => "confirmed",
            WellKnownResponse::Text(_) => "text",
            WellKnownResponse::Selected(_) => "selected",
            WellKnownResponse::Cancelled(_) => "cancelled",
        }
    }
}

// =============================================================================
// Legacy Types (Deprecated - for backwards compatibility)
// =============================================================================

/// Standard request types for common interactive UI patterns.
///
/// **DEPRECATED**: This enum-based type is maintained for backwards compatibility.
/// New code should use the trait-based types instead:
/// - [`ConfirmRequest`] instead of `StandardRequest::Confirm`
/// - [`PromptRequest`] instead of `StandardRequest::Prompt`
/// - [`SelectRequest`] instead of `StandardRequest::Select`
/// - Or implement [`BidirRequest`] for custom types
///
/// These request types cover the most common server-to-client interactions:
///
/// - **Confirm**: Yes/no questions before important actions
/// - **Prompt**: Free-form text input from the user
/// - **Select**: Choose one or more options from a list
/// - **Custom**: Domain-specific request payload
///
/// The type parameter `T` defaults to [`serde_json::Value`] for backwards compatibility.
/// Use a custom type for domain-specific interactions.
///
/// For domain-specific interactions (e.g., image quality selection, custom
/// dialogs), define your own request/response types and implement the
/// [`BidirRequest`] trait.
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
/// | `custom` | Application-defined |
#[deprecated(
    since = "0.2.0",
    note = "Use ConfirmRequest, PromptRequest, SelectRequest, or implement BidirRequest for custom types"
)]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    bound(
        serialize = "T: Serialize",
        deserialize = "T: serde::de::DeserializeOwned"
    )
)]
pub enum StandardRequest<T = serde_json::Value>
where
    T: Serialize + DeserializeOwned + JsonSchema,
{
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
        default: Option<T>,

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
        options: Vec<SelectOption<T>>,

        /// Whether to allow selecting multiple options.
        /// - `false` (default): single selection, returns one value
        /// - `true`: multiple selection, returns zero or more values
        #[serde(default)]
        multi_select: bool,
    },

    /// Custom domain-specific request payload.
    ///
    /// Use this for application-specific interactions that don't fit
    /// the standard confirm/prompt/select patterns.
    Custom {
        /// The custom request data.
        data: T,
    },
}

/// Standard response types matching [`StandardRequest`].
///
/// **DEPRECATED**: This enum-based type is maintained for backwards compatibility.
/// New code should use the trait-based types instead:
/// - [`ConfirmedResponse`] instead of `StandardResponse::Confirmed`
/// - [`TextResponse`] instead of `StandardResponse::Text`
/// - [`SelectedResponse`] instead of `StandardResponse::Selected`
/// - [`CancelledResponse`] instead of `StandardResponse::Cancelled`
/// - Or implement [`BidirResponse`] for custom types
///
/// Each variant corresponds to a request type:
///
/// | Request | Response |
/// |---------|----------|
/// | `Confirm` | `Confirmed { value: bool }` |
/// | `Prompt` | `Text { value: T }` |
/// | `Select` | `Selected { values: Vec<T> }` |
/// | Any | `Cancelled` (user cancelled) |
///
/// The type parameter `T` defaults to [`serde_json::Value`] for backwards compatibility.
///
/// # Wire Format
///
/// Uses internally-tagged JSON for consistency with TypeScript clients:
/// ```json
/// { "type": "confirmed", "value": true }
/// { "type": "text", "value": "user-input" }
/// { "type": "selected", "values": ["dev"] }
/// { "type": "cancelled" }
/// ```
#[deprecated(
    since = "0.2.0",
    note = "Use ConfirmedResponse, TextResponse, SelectedResponse, CancelledResponse, or implement BidirResponse for custom types"
)]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    bound(
        serialize = "T: Serialize",
        deserialize = "T: serde::de::DeserializeOwned"
    )
)]
pub enum StandardResponse<T = serde_json::Value>
where
    T: Serialize + DeserializeOwned + JsonSchema,
{
    /// User confirmed (true) or declined (false).
    ///
    /// Response to `StandardRequest::Confirm`.
    /// - `value: true` = user said yes
    /// - `value: false` = user said no
    Confirmed {
        /// Whether the user confirmed (true) or declined (false)
        value: bool,
    },

    /// User entered text or provided a value.
    ///
    /// Response to `StandardRequest::Prompt`.
    /// May be empty if user submitted without entering text.
    Text {
        /// The value entered or provided by the user
        value: T,
    },

    /// User selected one or more options (by value).
    ///
    /// Response to `StandardRequest::Select`.
    /// Contains the `value` field(s) from selected [`SelectOption`]s.
    ///
    /// - For single-select: vector with exactly one element
    /// - For multi-select: vector with zero or more elements
    Selected {
        /// The values of selected options
        values: Vec<T>,
    },

    /// Custom domain-specific response payload.
    ///
    /// Corresponds to `StandardRequest::Custom` or any request type
    /// where the application needs to return a custom response.
    Custom {
        /// The custom response data.
        data: T,
    },

    /// User cancelled the request.
    ///
    /// Can be sent in response to any request type.
    /// Indicates the user chose to abort rather than respond.
    /// This is different from declining (Confirmed { value: false }) - cancel
    /// means "don't proceed with the workflow at all".
    Cancelled,
}

/// An option in a [`StandardRequest::Select`] request.
///
/// Each option has:
/// - **value**: Machine-readable identifier returned in the response (generic over `T`)
/// - **label**: Human-readable text displayed to the user
/// - **description**: Optional additional context about the option
///
/// The type parameter `T` defaults to [`serde_json::Value`] for backwards compatibility.
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
#[serde(bound(
    serialize = "T: Serialize",
    deserialize = "T: serde::de::DeserializeOwned"
))]
pub struct SelectOption<T = serde_json::Value>
where
    T: Serialize + DeserializeOwned + JsonSchema,
{
    /// Machine-readable value returned when this option is selected.
    ///
    /// This is what appears in `StandardResponse::Selected`.
    /// Should be a stable identifier (e.g., "dev", "prod", "option_1").
    pub value: T,

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
    /// This constructor is for the default `T = serde_json::Value` type.
    /// The `value` parameter is converted to `serde_json::Value` via `.into()`.
    ///
    /// # Arguments
    ///
    /// * `value` - Machine-readable identifier (returned in response), convertible to `serde_json::Value`
    /// * `label` - Human-readable display text
    ///
    /// # Example
    ///
    /// ```rust
    /// use plexus_core::plexus::bidirectional::SelectOption;
    ///
    /// let opt = SelectOption::new("dev", "Development Environment");
    /// assert_eq!(opt.label, "Development Environment");
    /// ```
    pub fn new(value: impl Into<serde_json::Value>, label: impl Into<String>) -> Self {
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

impl<T> SelectOption<T>
where
    T: Serialize + DeserializeOwned + JsonSchema,
{
    /// Create a new select option with a typed value and label.
    ///
    /// Use this constructor when working with a custom type `T`.
    ///
    /// # Arguments
    ///
    /// * `value` - The typed value for this option
    /// * `label` - Human-readable display text
    pub fn new_typed(value: T, label: impl Into<String>) -> Self {
        Self {
            value,
            label: label.into(),
            description: None,
        }
    }

    /// Add a description to this option.
    pub fn with_description_typed(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_request_serialization() {
        let req: StandardRequest = StandardRequest::Confirm {
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
        let resp: StandardResponse = StandardResponse::Confirmed { value: true };

        let json = serde_json::to_value(&resp).unwrap();
        // Internally tagged: { "type": "confirmed", "value": true }
        assert_eq!(json["type"], "confirmed");
        assert_eq!(json["value"], true);

        // Test round-trip
        let roundtrip: StandardResponse = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, StandardResponse::Confirmed { value: true });
    }

    #[test]
    fn test_select_option_builder() {
        let opt = SelectOption::new("prod", "Production")
            .with_description("Requires approval");

        assert_eq!(opt.value, serde_json::Value::String("prod".into()));
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

    #[test]
    fn test_standard_request_prompt_generic() {
        let req: StandardRequest = StandardRequest::Prompt {
            message: "Enter value:".into(),
            default: Some(serde_json::Value::String("default".into())),
            placeholder: None,
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["type"], "prompt");
        assert_eq!(json["default"], "default");
    }

    #[test]
    fn test_standard_response_text_generic() {
        let resp: StandardResponse = StandardResponse::Text {
            value: serde_json::Value::String("hello".into()),
        };

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["value"], "hello");

        let roundtrip: StandardResponse = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, resp);
    }

    #[test]
    fn test_standard_response_selected_generic() {
        let resp: StandardResponse = StandardResponse::Selected {
            values: vec![
                serde_json::Value::String("a".into()),
                serde_json::Value::String("b".into()),
            ],
        };

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "selected");
        assert!(json["values"].as_array().unwrap().len() == 2);
    }

    #[test]
    fn test_custom_variant_request() {
        let req: StandardRequest = StandardRequest::Custom {
            data: serde_json::json!({"action": "special", "param": 42}),
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["type"], "custom");
        assert_eq!(json["data"]["action"], "special");
    }

    #[test]
    fn test_custom_variant_response() {
        let resp: StandardResponse = StandardResponse::Custom {
            data: serde_json::json!({"result": "ok"}),
        };

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "custom");
        assert_eq!(json["data"]["result"], "ok");
    }

    // =============================================================================
    // Tests for new trait-based types
    // =============================================================================

    #[test]
    fn test_confirm_request() {
        let req = ConfirmRequest {
            message: "Delete file?".into(),
            default: Some(false),
        };

        assert_eq!(req.type_tag(), "confirm");

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["message"], "Delete file?");
        assert_eq!(json["default"], false);

        let roundtrip: ConfirmRequest = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, req);
    }

    #[test]
    fn test_prompt_request() {
        let req = PromptRequest {
            message: "Enter name:".into(),
            default: Some("John".into()),
            placeholder: Some("Your name".into()),
        };

        assert_eq!(req.type_tag(), "prompt");

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["message"], "Enter name:");
        assert_eq!(json["default"], "John");
        assert_eq!(json["placeholder"], "Your name");

        let roundtrip: PromptRequest = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, req);
    }

    #[test]
    fn test_select_request() {
        let req = SelectRequest {
            message: "Choose option:".into(),
            options: vec![
                SelectOption::new("a", "Option A"),
                SelectOption::new("b", "Option B").with_description("Second option"),
            ],
            multi_select: false,
        };

        assert_eq!(req.type_tag(), "select");

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["message"], "Choose option:");
        assert_eq!(json["options"][0]["value"], "a");
        assert_eq!(json["options"][0]["label"], "Option A");
        assert_eq!(json["options"][1]["description"], "Second option");
        assert_eq!(json["multiSelect"], false);

        let roundtrip: SelectRequest = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, req);
    }

    #[test]
    fn test_confirmed_response() {
        let resp = ConfirmedResponse { value: true };

        assert_eq!(resp.type_tag(), "confirmed");

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["value"], true);

        let roundtrip: ConfirmedResponse = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, resp);
    }

    #[test]
    fn test_text_response() {
        let resp = TextResponse {
            value: "Hello".into(),
        };

        assert_eq!(resp.type_tag(), "text");

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["value"], "Hello");

        let roundtrip: TextResponse = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, resp);
    }

    #[test]
    fn test_selected_response() {
        let resp = SelectedResponse {
            values: vec!["a".into(), "b".into()],
        };

        assert_eq!(resp.type_tag(), "selected");

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["values"][0], "a");
        assert_eq!(json["values"][1], "b");

        let roundtrip: SelectedResponse = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, resp);
    }

    #[test]
    fn test_cancelled_response() {
        let resp = CancelledResponse {};

        assert_eq!(resp.type_tag(), "cancelled");

        let json = serde_json::to_value(&resp).unwrap();
        // Should be an empty object or just type tag
        let roundtrip: CancelledResponse = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, resp);
    }

    #[test]
    fn test_well_known_request_enum() {
        let req = WellKnownRequest::Confirm(ConfirmRequest {
            message: "Test?".into(),
            default: None,
        });

        assert_eq!(req.type_tag(), "confirm");

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["type"], "confirm");
        assert_eq!(json["message"], "Test?");

        let roundtrip: WellKnownRequest = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, req);
    }

    #[test]
    fn test_well_known_response_enum() {
        let resp = WellKnownResponse::Confirmed(ConfirmedResponse { value: true });

        assert_eq!(resp.type_tag(), "confirmed");

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["type"], "confirmed");
        assert_eq!(json["value"], true);

        let roundtrip: WellKnownResponse = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, resp);
    }
}
