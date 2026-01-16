//! WebSocket transport types for bidirectional streaming.
//!
//! This module provides the message types for bidirectional communication
//! over WebSocket connections. Unlike MCP which requires workarounds,
//! WebSocket natively supports true bidirectional communication.
//!
//! # Architecture
//!
//! ```text
//! ┌───────────────────────────────────────────────────────────────────────┐
//! │ 1. Client subscribes: plexus_subscribe { method: "sync", params: {} } │
//! │                                                                       │
//! │ 2. Server streams via subscription:                                   │
//! │    - { type: "progress", message: "Processing..." }                   │
//! │    - { type: "request", request_id: "abc", request_type: "confirm" }  │
//! │                                                                       │
//! │ 3. Client sends via plexus_respond RPC:                               │
//! │    - plexus_respond(sub_id, request_id, payload)                      │
//! │                                                                       │
//! │ 4. Server continues streaming:                                        │
//! │    - { type: "data", content: {...} }                                 │
//! │    - { type: "done" }                                                 │
//! └───────────────────────────────────────────────────────────────────────┘
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::plexus::types::{PlexusStreamItem, RequestType, ResponsePayload};

// =============================================================================
// Server → Client Messages
// =============================================================================

/// Messages sent from server to client during a WebSocket subscription.
///
/// These are the wire-format messages that flow through the subscription sink.
/// They are a simplified view of `PlexusStreamItem` without internal metadata.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SubscriptionMessage {
    /// Progress update during long-running operations.
    Progress {
        /// Human-readable progress message.
        message: String,
        /// Optional completion percentage (0.0 - 100.0).
        #[serde(skip_serializing_if = "Option::is_none")]
        percentage: Option<f32>,
    },

    /// Data payload from the activation.
    Data {
        /// Type identifier for deserialization (e.g., "health.status").
        content_type: String,
        /// The actual payload data.
        content: Value,
    },

    /// Server requests client input (bidirectional communication).
    Request {
        /// Unique identifier for correlating response.
        request_id: String,
        /// Type of request and its parameters.
        request_type: RequestType,
        /// Optional timeout in milliseconds.
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },

    /// Error occurred during processing.
    Error {
        /// Human-readable error message.
        message: String,
        /// Optional error code for programmatic handling.
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
        /// Whether the operation can be retried.
        recoverable: bool,
    },

    /// Stream completed successfully.
    Done,
}

impl From<PlexusStreamItem> for SubscriptionMessage {
    fn from(item: PlexusStreamItem) -> Self {
        match item {
            PlexusStreamItem::Progress {
                message,
                percentage,
                ..
            } => SubscriptionMessage::Progress {
                message,
                percentage,
            },
            PlexusStreamItem::Data {
                content_type,
                content,
                ..
            } => SubscriptionMessage::Data {
                content_type,
                content,
            },
            PlexusStreamItem::Request {
                request_id,
                request_type,
                timeout_ms,
                ..
            } => SubscriptionMessage::Request {
                request_id,
                request_type,
                timeout_ms,
            },
            PlexusStreamItem::Error {
                message,
                code,
                recoverable,
                ..
            } => SubscriptionMessage::Error {
                message,
                code,
                recoverable,
            },
            PlexusStreamItem::Done { .. } => SubscriptionMessage::Done,
        }
    }
}

// =============================================================================
// Client → Server Messages
// =============================================================================

/// Messages sent from client to server during a WebSocket subscription.
///
/// These are used to respond to server requests or cancel operations.
/// Sent via the `plexus_respond` RPC method rather than inline in the subscription.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Response to a server request.
    Response {
        /// Request ID this response correlates to.
        request_id: String,
        /// The response payload.
        payload: ResponsePayload,
    },

    /// Client-initiated cancellation of the subscription.
    Cancel,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plexus::types::StreamMetadata;

    #[test]
    fn test_subscription_message_progress_serialization() {
        let msg = SubscriptionMessage::Progress {
            message: "Processing...".into(),
            percentage: Some(50.0),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"progress\""));
        assert!(json.contains("\"message\":\"Processing...\""));
        assert!(json.contains("\"percentage\":50.0"));
    }

    #[test]
    fn test_subscription_message_data_serialization() {
        let msg = SubscriptionMessage::Data {
            content_type: "health.status".into(),
            content: serde_json::json!({ "status": "healthy" }),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"data\""));
        assert!(json.contains("\"content_type\":\"health.status\""));
    }

    #[test]
    fn test_subscription_message_request_serialization() {
        let msg = SubscriptionMessage::Request {
            request_id: "req-123".into(),
            request_type: RequestType::Confirm {
                message: "Continue?".into(),
                default: Some(true),
            },
            timeout_ms: Some(30000),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"request\""));
        assert!(json.contains("\"request_id\":\"req-123\""));
        assert!(json.contains("\"timeout_ms\":30000"));
    }

    #[test]
    fn test_subscription_message_error_serialization() {
        let msg = SubscriptionMessage::Error {
            message: "Something went wrong".into(),
            code: Some("E001".into()),
            recoverable: false,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"error\""));
        assert!(json.contains("\"message\":\"Something went wrong\""));
        assert!(json.contains("\"code\":\"E001\""));
        assert!(json.contains("\"recoverable\":false"));
    }

    #[test]
    fn test_subscription_message_done_serialization() {
        let msg = SubscriptionMessage::Done;

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"done\""));
    }

    #[test]
    fn test_client_message_response_serialization() {
        // Note: ResponsePayload::Confirmed(bool) and similar newtype variants
        // cannot be serialized with internally tagged serde due to serde limitations.
        // We test with Cancelled which has no payload, or test deserialization.
        let msg = ClientMessage::Response {
            request_id: "req-123".into(),
            payload: ResponsePayload::Cancelled,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"response\""));
        assert!(json.contains("\"request_id\":\"req-123\""));
        assert!(json.contains("\"cancelled\""));
    }

    #[test]
    fn test_client_message_cancel_serialization() {
        let msg = ClientMessage::Cancel;

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"cancel\""));
    }

    #[test]
    fn test_plexus_stream_item_to_subscription_message() {
        let metadata = StreamMetadata::new(vec!["plexus".into()], "hash".into());

        // Test Progress conversion
        let item = PlexusStreamItem::progress(metadata.clone(), "Working...".into(), Some(25.0));
        let msg: SubscriptionMessage = item.into();
        assert!(matches!(
            msg,
            SubscriptionMessage::Progress {
                message,
                percentage: Some(25.0),
                ..
            } if message == "Working..."
        ));

        // Test Data conversion
        let item = PlexusStreamItem::data(
            metadata.clone(),
            "test.type".into(),
            serde_json::json!({"key": "value"}),
        );
        let msg: SubscriptionMessage = item.into();
        assert!(matches!(
            msg,
            SubscriptionMessage::Data { content_type, .. } if content_type == "test.type"
        ));

        // Test Done conversion
        let item = PlexusStreamItem::done(metadata.clone());
        let msg: SubscriptionMessage = item.into();
        assert!(matches!(msg, SubscriptionMessage::Done));

        // Test Error conversion
        let item = PlexusStreamItem::error(
            metadata.clone(),
            "Error!".into(),
            Some("E001".into()),
            false,
        );
        let msg: SubscriptionMessage = item.into();
        assert!(matches!(
            msg,
            SubscriptionMessage::Error {
                message,
                code: Some(c),
                recoverable: false,
            } if message == "Error!" && c == "E001"
        ));
    }

    #[test]
    fn test_roundtrip_serialization() {
        let msg = SubscriptionMessage::Request {
            request_id: "test-id".into(),
            request_type: RequestType::Prompt {
                message: "Enter name:".into(),
                default: None,
                placeholder: Some("Your name".into()),
            },
            timeout_ms: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let parsed: SubscriptionMessage = serde_json::from_str(&json).unwrap();

        match parsed {
            SubscriptionMessage::Request {
                request_id,
                request_type,
                timeout_ms,
            } => {
                assert_eq!(request_id, "test-id");
                assert!(timeout_ms.is_none());
                match request_type {
                    RequestType::Prompt {
                        message,
                        default,
                        placeholder,
                    } => {
                        assert_eq!(message, "Enter name:");
                        assert!(default.is_none());
                        assert_eq!(placeholder, Some("Your name".into()));
                    }
                    _ => panic!("Wrong request type"),
                }
            }
            _ => panic!("Wrong message type"),
        }
    }
}
