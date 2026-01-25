//! Structured error codes and categories for Plexus errors
//!
//! This module provides a standardized error code system following JSON-RPC 2.0
//! conventions with Plexus-specific extensions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Structured error codes following JSON-RPC 2.0 + Plexus extensions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    // Standard JSON-RPC 2.0 codes (-32700 to -32600)
    /// Invalid JSON was received by the server (-32700)
    ParseError,

    /// The JSON sent is not a valid Request object (-32600)
    InvalidRequest,

    /// The method does not exist / is not available (-32601)
    MethodNotFound,

    /// Invalid method parameter(s) (-32602)
    InvalidParams,

    /// Internal JSON-RPC error (-32603)
    InternalError,

    // Plexus application codes (custom range: -32000 to -32099)
    /// Parameter validation failed (-32001)
    ValidationError,

    /// Resource not found (-32002)
    NotFound,

    /// Resource already exists (-32003)
    AlreadyExists,

    /// Authentication required (-32004)
    Unauthorized,

    /// Permission denied (-32005)
    Forbidden,

    /// State conflict (-32006)
    Conflict,

    /// Operation timed out (-32007)
    Timeout,

    /// Feature not supported (-32008)
    Unsupported,

    /// Rate limit exceeded (-32009)
    RateLimited,

    /// Activation/namespace not found (-32010)
    ActivationNotFound,

    /// Handle format invalid (-32011)
    HandleInvalid,

    /// Handle resolution not supported (-32012)
    HandleNotSupported,

    /// Invalid state for operation (-32013)
    StateError,
}

impl ErrorCode {
    /// Get numeric code for JSON-RPC compatibility
    pub fn to_i32(&self) -> i32 {
        match self {
            // Standard JSON-RPC codes
            ErrorCode::ParseError => -32700,
            ErrorCode::InvalidRequest => -32600,
            ErrorCode::MethodNotFound => -32601,
            ErrorCode::InvalidParams => -32602,
            ErrorCode::InternalError => -32603,

            // Plexus application codes
            ErrorCode::ValidationError => -32001,
            ErrorCode::NotFound => -32002,
            ErrorCode::AlreadyExists => -32003,
            ErrorCode::Unauthorized => -32004,
            ErrorCode::Forbidden => -32005,
            ErrorCode::Conflict => -32006,
            ErrorCode::Timeout => -32007,
            ErrorCode::Unsupported => -32008,
            ErrorCode::RateLimited => -32009,
            ErrorCode::ActivationNotFound => -32010,
            ErrorCode::HandleInvalid => -32011,
            ErrorCode::HandleNotSupported => -32012,
            ErrorCode::StateError => -32013,
        }
    }

    /// Get error category for this error code
    pub fn category(&self) -> ErrorCategory {
        match self {
            ErrorCode::InvalidParams | ErrorCode::ValidationError => ErrorCategory::Validation,
            ErrorCode::NotFound | ErrorCode::ActivationNotFound => ErrorCategory::NotFound,
            ErrorCode::Unauthorized => ErrorCategory::Authentication,
            ErrorCode::Forbidden => ErrorCategory::Authorization,
            ErrorCode::AlreadyExists | ErrorCode::Conflict | ErrorCode::StateError => {
                ErrorCategory::Conflict
            }
            ErrorCode::RateLimited => ErrorCategory::RateLimit,
            ErrorCode::Timeout => ErrorCategory::Timeout,
            ErrorCode::Unsupported | ErrorCode::HandleNotSupported => ErrorCategory::Unsupported,
            _ => ErrorCategory::Internal,
        }
    }

    /// Is this error recoverable by default?
    ///
    /// Recoverable errors can typically be retried after correcting the issue.
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            ErrorCode::Timeout | ErrorCode::RateLimited | ErrorCode::Conflict
        )
    }
}

/// Error category for programmatic handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// Invalid parameters or validation failure
    Validation,

    /// Resource doesn't exist
    NotFound,

    /// Authentication required
    Authentication,

    /// Permission denied
    Authorization,

    /// State conflict or resource already exists
    Conflict,

    /// Rate limit exceeded
    RateLimit,

    /// Server error
    Internal,

    /// Network/transport error
    Network,

    /// Operation timed out
    Timeout,

    /// Feature not supported
    Unsupported,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_to_i32() {
        assert_eq!(ErrorCode::InvalidParams.to_i32(), -32602);
        assert_eq!(ErrorCode::MethodNotFound.to_i32(), -32601);
        assert_eq!(ErrorCode::NotFound.to_i32(), -32002);
        assert_eq!(ErrorCode::ValidationError.to_i32(), -32001);
    }

    #[test]
    fn test_error_code_category() {
        assert_eq!(
            ErrorCode::InvalidParams.category(),
            ErrorCategory::Validation
        );
        assert_eq!(ErrorCode::NotFound.category(), ErrorCategory::NotFound);
        assert_eq!(ErrorCode::Timeout.category(), ErrorCategory::Timeout);
        assert_eq!(
            ErrorCode::Unauthorized.category(),
            ErrorCategory::Authentication
        );
    }

    #[test]
    fn test_error_code_recoverable() {
        assert!(ErrorCode::Timeout.is_recoverable());
        assert!(ErrorCode::RateLimited.is_recoverable());
        assert!(!ErrorCode::NotFound.is_recoverable());
        assert!(!ErrorCode::ValidationError.is_recoverable());
    }

    #[test]
    fn test_error_code_serialization() {
        let code = ErrorCode::InvalidParams;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "\"INVALID_PARAMS\"");

        let deserialized: ErrorCode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ErrorCode::InvalidParams);
    }

    #[test]
    fn test_error_category_serialization() {
        let category = ErrorCategory::Validation;
        let json = serde_json::to_string(&category).unwrap();
        assert_eq!(json, "\"validation\"");

        let deserialized: ErrorCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ErrorCategory::Validation);
    }
}
