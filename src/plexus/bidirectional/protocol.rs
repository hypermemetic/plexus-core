//! Trait-based protocol for bidirectional communication
//!
//! This module defines the core traits for the bidirectional protocol:
//! - [`BidirRequest`] - Trait for all request types
//! - [`BidirResponse`] - Trait for all response types
//!
//! # Design Philosophy
//!
//! The trait-based protocol eliminates the need for enum wrapping:
//!
//! **Old (enum-based)**:
//! ```rust,ignore
//! enum Request<T> {
//!     Confirm { ... },
//!     Prompt { ... },
//!     Custom { data: T },
//! }
//! // Forces users to match all cases even if they only use one
//! ```
//!
//! **New (trait-based)**:
//! ```rust,ignore
//! trait BidirRequest: Serialize + DeserializeOwned + Send + 'static {
//!     fn type_tag(&self) -> &'static str;
//! }
//!
//! struct ConfirmRequest { message: String, default: Option<bool> }
//! impl BidirRequest for ConfirmRequest { ... }
//! // Users work with plain structs directly
//! ```
//!
//! # Type Tags
//!
//! Each request/response type must provide a `type_tag` for JSON discrimination.
//! This enables proper serialization/deserialization across the wire:
//!
//! ```json
//! {
//!   "type": "confirm",
//!   "message": "Delete file?",
//!   "default": false
//! }
//! ```
//!
//! # Implementation Example
//!
//! ```rust,ignore
//! use plexus_core::plexus::bidirectional::protocol::{BidirRequest, BidirResponse};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct CustomRequest {
//!     pub action: String,
//!     pub params: serde_json::Value,
//! }
//!
//! impl BidirRequest for CustomRequest {
//!     fn type_tag(&self) -> &'static str {
//!         "custom_request"
//!     }
//! }
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct CustomResponse {
//!     pub result: String,
//! }
//!
//! impl BidirResponse for CustomResponse {
//!     fn type_tag(&self) -> &'static str {
//!         "custom_response"
//!     }
//! }
//! ```

use serde::de::DeserializeOwned;
use serde::Serialize;

/// Trait for bidirectional request types
///
/// Implement this trait for any type that can be used as a request in
/// bidirectional communication. The type must be serializable, deserializable,
/// and safe to send across thread boundaries.
///
/// # Type Tag
///
/// The `type_tag()` method provides a discriminator for JSON serialization.
/// This tag is added to the JSON object to enable proper deserialization on
/// the receiving end.
///
/// # Example
///
/// ```rust,ignore
/// use plexus_core::plexus::bidirectional::protocol::BidirRequest;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// pub struct ConfirmRequest {
///     pub message: String,
///     pub default: Option<bool>,
/// }
///
/// impl BidirRequest for ConfirmRequest {
///     fn type_tag(&self) -> &'static str {
///         "confirm"
///     }
/// }
/// ```
pub trait BidirRequest: Serialize + DeserializeOwned + Send + 'static {
    /// Returns the type tag for JSON discrimination
    ///
    /// This tag is added to the serialized JSON object to enable proper
    /// deserialization and routing on the receiving end.
    ///
    /// # Requirements
    ///
    /// - Must be a unique string for each request type
    /// - Should be lowercase and use underscores (e.g., "confirm", "my_custom_request")
    /// - Must be a static string (no dynamic allocation)
    fn type_tag(&self) -> &'static str;
}

/// Trait for bidirectional response types
///
/// Implement this trait for any type that can be used as a response in
/// bidirectional communication. The type must be serializable, deserializable,
/// and safe to send across thread boundaries.
///
/// # Type Tag
///
/// Similar to [`BidirRequest`], the `type_tag()` method provides a discriminator
/// for JSON serialization.
///
/// # Example
///
/// ```rust,ignore
/// use plexus_core::plexus::bidirectional::protocol::BidirResponse;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// pub struct ConfirmedResponse {
///     pub value: bool,
/// }
///
/// impl BidirResponse for ConfirmedResponse {
///     fn type_tag(&self) -> &'static str {
///         "confirmed"
///     }
/// }
/// ```
pub trait BidirResponse: Serialize + DeserializeOwned + Send + 'static {
    /// Returns the type tag for JSON discrimination
    ///
    /// This tag is added to the serialized JSON object to enable proper
    /// deserialization and routing on the receiving end.
    ///
    /// # Requirements
    ///
    /// - Must be a unique string for each response type
    /// - Should be lowercase and use underscores (e.g., "confirmed", "my_custom_response")
    /// - Must be a static string (no dynamic allocation)
    fn type_tag(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestRequest {
        message: String,
    }

    impl BidirRequest for TestRequest {
        fn type_tag(&self) -> &'static str {
            "test_request"
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestResponse {
        result: String,
    }

    impl BidirResponse for TestResponse {
        fn type_tag(&self) -> &'static str {
            "test_response"
        }
    }

    #[test]
    fn test_request_trait() {
        let req = TestRequest {
            message: "Hello".to_string(),
        };
        assert_eq!(req.type_tag(), "test_request");

        // Test serialization
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["message"], "Hello");

        // Test deserialization
        let roundtrip: TestRequest = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, req);
    }

    #[test]
    fn test_response_trait() {
        let resp = TestResponse {
            result: "OK".to_string(),
        };
        assert_eq!(resp.type_tag(), "test_response");

        // Test serialization
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["result"], "OK");

        // Test deserialization
        let roundtrip: TestResponse = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, resp);
    }
}
