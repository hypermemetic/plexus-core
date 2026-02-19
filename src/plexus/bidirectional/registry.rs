//! Global pending response registry for bidirectional communication
//!
//! This module provides a global registry for pending bidirectional requests,
//! enabling transports like MCP (which are fundamentally request-response) to
//! route responses back to the correct BidirChannel.
//!
//! # Architecture
//!
//! 1. When a BidirChannel sends a request, it registers a callback in this registry
//! 2. The transport (e.g., MCP) sends the request to the client as a notification
//! 3. The client responds via a tool call (e.g., `_plexus_respond`)
//! 4. The transport looks up the request in this registry and forwards the response
//! 5. The registry callback deserializes and sends to the waiting BidirChannel
//!
//! # Thread Safety
//!
//! The registry uses a RwLock for concurrent read access with exclusive write access.
//! Registrations and lookups are fast; response handling is done outside the lock.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};
use tokio::sync::oneshot;

use super::types::BidirError;

/// Type alias for the response sender (type-erased to Value)
type ResponseSender = oneshot::Sender<Value>;

/// Global registry for pending bidirectional requests
///
/// This registry allows transports to correlate response messages with
/// the BidirChannel waiting for them.
static PENDING_RESPONSES: LazyLock<RwLock<HashMap<String, ResponseSender>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Register a pending request in the global registry
///
/// Called by BidirChannel when making a request over a transport that
/// doesn't natively support bidirectional (like MCP).
///
/// # Arguments
///
/// * `request_id` - Unique identifier for the request
/// * `sender` - Oneshot channel sender to forward the response
///
/// # Example
///
/// ```rust,ignore
/// let (tx, rx) = oneshot::channel();
/// register_pending_request("req-123", tx);
/// // Transport sends request...
/// // Later, _plexus_respond calls handle_pending_response("req-123", value)
/// let response = rx.await?;
/// ```
pub fn register_pending_request(request_id: String, sender: ResponseSender) {
    let mut registry = PENDING_RESPONSES.write().unwrap();
    registry.insert(request_id, sender);
}

/// Remove a pending request from the registry (e.g., on timeout)
///
/// # Arguments
///
/// * `request_id` - The request ID to remove
///
/// # Returns
///
/// The removed sender if it existed, or None
pub fn unregister_pending_request(request_id: &str) -> Option<ResponseSender> {
    let mut registry = PENDING_RESPONSES.write().unwrap();
    registry.remove(request_id)
}

/// Handle a response for a pending request
///
/// Called by transport tools like `_plexus_respond` when receiving a client response.
///
/// # Arguments
///
/// * `request_id` - The request ID from the client's response
/// * `response_data` - The JSON response data
///
/// # Returns
///
/// * `Ok(())` if the response was successfully forwarded
/// * `Err(BidirError::UnknownRequest)` if no pending request with that ID
/// * `Err(BidirError::ChannelClosed)` if the receiver was dropped (timeout/cancelled)
///
/// # Example
///
/// ```rust,ignore
/// // In _plexus_respond tool handler:
/// let result = handle_pending_response(request_id, response_data)?;
/// ```
pub fn handle_pending_response(request_id: &str, response_data: Value) -> Result<(), BidirError> {
    // Remove from registry (takes ownership of sender)
    let sender = {
        let mut registry = PENDING_RESPONSES.write().unwrap();
        registry.remove(request_id)
    };

    match sender {
        Some(tx) => {
            // Send response through channel
            tx.send(response_data).map_err(|_| BidirError::ChannelClosed)
        }
        None => Err(BidirError::UnknownRequest),
    }
}

/// Check if a request is pending
///
/// # Arguments
///
/// * `request_id` - The request ID to check
///
/// # Returns
///
/// `true` if a request with this ID is pending, `false` otherwise
pub fn is_request_pending(request_id: &str) -> bool {
    let registry = PENDING_RESPONSES.read().unwrap();
    registry.contains_key(request_id)
}

/// Get the count of pending requests (for monitoring/debugging)
pub fn pending_count() -> usize {
    let registry = PENDING_RESPONSES.read().unwrap();
    registry.len()
}

/// Clear all pending requests (for testing)
#[cfg(test)]
#[allow(dead_code)]
pub fn clear_all() {
    let mut registry = PENDING_RESPONSES.write().unwrap();
    registry.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests run concurrently and share a global registry.
    // Use unique request IDs and assert on per-ID presence (is_request_pending)
    // rather than global pending_count(), which races with concurrent tests.

    #[tokio::test]
    async fn test_register_and_handle() {
        let (tx, rx) = oneshot::channel();
        let request_id = format!("test-reg-handle-{}", uuid::Uuid::new_v4());

        // Register
        register_pending_request(request_id.clone(), tx);
        assert!(is_request_pending(&request_id));

        // Handle response
        let response = serde_json::json!({"confirmed": true});
        handle_pending_response(&request_id, response.clone()).unwrap();

        // Verify response received
        let received = rx.await.unwrap();
        assert_eq!(received, response);

        // Request should be removed
        assert!(!is_request_pending(&request_id));
    }

    #[tokio::test]
    async fn test_unknown_request() {
        let result = handle_pending_response(
            &format!("nonexistent-{}", uuid::Uuid::new_v4()),
            serde_json::json!({}),
        );
        assert!(matches!(result, Err(BidirError::UnknownRequest)));
    }

    #[tokio::test]
    async fn test_unregister() {
        let (tx, _rx) = oneshot::channel();
        let request_id = format!("test-unreg-{}", uuid::Uuid::new_v4());

        register_pending_request(request_id.clone(), tx);
        assert!(is_request_pending(&request_id));

        let removed = unregister_pending_request(&request_id);
        assert!(removed.is_some());
        assert!(!is_request_pending(&request_id));
    }

    #[tokio::test]
    async fn test_channel_closed() {
        let (tx, rx) = oneshot::channel();
        let request_id = format!("test-closed-{}", uuid::Uuid::new_v4());

        register_pending_request(request_id.clone(), tx);

        // Drop the receiver
        drop(rx);

        // Handle should fail with ChannelClosed
        let result = handle_pending_response(&request_id, serde_json::json!({}));
        assert!(matches!(result, Err(BidirError::ChannelClosed)));
    }
}
