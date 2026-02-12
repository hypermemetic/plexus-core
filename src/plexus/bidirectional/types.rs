//! Bidirectional streaming types
//!
//! Enables server-to-client requests during streaming RPC execution.
//! Fully generic design allows activations to define custom request/response types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Error types for bidirectional communication
#[derive(Debug, Clone, thiserror::Error)]
pub enum BidirError {
    /// Transport does not support bidirectional communication
    #[error("Bidirectional communication not supported by this transport")]
    NotSupported,

    /// Request timed out waiting for client response
    #[error("Request timed out after {0}ms")]
    Timeout(u64),

    /// Client explicitly cancelled the request
    #[error("Request was cancelled by client")]
    Cancelled,

    /// Response type doesn't match expected type
    #[error("Type mismatch: expected {expected}, got {got}")]
    TypeMismatch {
        /// Expected type name
        expected: String,
        /// Actual type received
        got: String,
    },

    /// Failed to serialize or deserialize request/response
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Transport-level error
    #[error("Transport error: {0}")]
    Transport(String),

    /// Unknown request ID (response for non-existent request)
    #[error("Unknown request ID")]
    UnknownRequest,

    /// Response channel was closed before response received
    #[error("Response channel closed")]
    ChannelClosed,
}

/// Standard request types for common interactive UI patterns
///
/// Use this for simple confirm/prompt/select interactions. For domain-specific
/// requests, define your own enum and use `BidirChannel<YourRequest, YourResponse>`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StandardRequest {
    /// Binary yes/no confirmation
    Confirm {
        /// Question to ask user
        message: String,
        /// Default answer if user just presses enter
        #[serde(skip_serializing_if = "Option::is_none")]
        default: Option<bool>,
    },

    /// Free-form text input
    Prompt {
        /// Prompt message
        message: String,
        /// Default value if user just presses enter
        #[serde(skip_serializing_if = "Option::is_none")]
        default: Option<String>,
        /// Placeholder text to show in input field
        #[serde(skip_serializing_if = "Option::is_none")]
        placeholder: Option<String>,
    },

    /// Select from options
    Select {
        /// Selection prompt
        message: String,
        /// Available options
        options: Vec<SelectOption>,
        /// Allow multiple selections
        #[serde(default)]
        multi_select: bool,
    },
}

/// Standard response types matching StandardRequest
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StandardResponse {
    /// User confirmed (true) or declined (false)
    Confirmed(bool),

    /// User entered text
    Text(String),

    /// User selected one or more options (by value)
    Selected(Vec<String>),

    /// User cancelled the request
    Cancelled,
}

/// Option in a Select request
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SelectOption {
    /// Option value (used in response)
    pub value: String,

    /// Display label shown to user
    pub label: String,

    /// Optional description for this option
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl SelectOption {
    /// Create a new select option
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            description: None,
        }
    }

    /// Add description to this option
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
