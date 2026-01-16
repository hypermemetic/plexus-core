//! Bidirectional communication support for activation methods.
//!
//! This module provides the context trait and extension methods for activations
//! that need to interact with clients (prompts, confirmations, selections, etc.)

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

// Re-export types from plexus types module for convenience
pub use super::types::{ClientResponse, RequestType, ResponsePayload, SelectOption};
use super::types::{PlexusStreamItem, StreamMetadata};

// ============================================================================
// Error Type
// ============================================================================

/// Error type for bidirectional operations
#[derive(Debug, Clone)]
pub enum BidirError {
    /// Client cancelled the request
    Cancelled,
    /// Request timed out waiting for response
    Timeout,
    /// Transport doesn't support bidirectional
    NotSupported,
    /// Response type doesn't match request type
    TypeMismatch { expected: String, got: String },
    /// Transport error
    Transport(String),
}

impl std::fmt::Display for BidirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "Request cancelled"),
            Self::Timeout => write!(f, "Request timed out"),
            Self::NotSupported => write!(f, "Bidirectional not supported"),
            Self::TypeMismatch { expected, got } => {
                write!(f, "Type mismatch: expected {}, got {}", expected, got)
            }
            Self::Transport(msg) => write!(f, "Transport error: {}", msg),
        }
    }
}

impl std::error::Error for BidirError {}

// ============================================================================
// Context Trait
// ============================================================================

/// Context for methods that support bidirectional communication
#[async_trait]
pub trait BidirectionalContext: Send + Sync {
    /// Send a request to the client and wait for response
    async fn request(&self, request_type: RequestType) -> Result<ResponsePayload, BidirError>;

    /// Send a request with custom timeout
    async fn request_with_timeout(
        &self,
        request_type: RequestType,
        timeout: Duration,
    ) -> Result<ResponsePayload, BidirError>;

    /// Check if bidirectional communication is supported by current transport
    fn is_bidirectional(&self) -> bool;
}

// ============================================================================
// Extension Trait for Common Patterns
// ============================================================================

/// Extension trait for common request patterns
#[async_trait]
pub trait BidirExt: BidirectionalContext {
    /// Simple yes/no confirmation
    async fn confirm(&self, message: impl Into<String> + Send) -> Result<bool, BidirError> {
        match self
            .request(RequestType::Confirm {
                message: message.into(),
                default: None,
            })
            .await?
        {
            ResponsePayload::Confirmed(v) => Ok(v),
            ResponsePayload::Cancelled => Err(BidirError::Cancelled),
            ResponsePayload::Timeout => Err(BidirError::Timeout),
            other => Err(BidirError::TypeMismatch {
                expected: "Confirmed".into(),
                got: format!("{:?}", other),
            }),
        }
    }

    /// Text input prompt
    async fn prompt(&self, message: impl Into<String> + Send) -> Result<String, BidirError> {
        match self
            .request(RequestType::Prompt {
                message: message.into(),
                default: None,
                placeholder: None,
            })
            .await?
        {
            ResponsePayload::Text(s) => Ok(s),
            ResponsePayload::Cancelled => Err(BidirError::Cancelled),
            ResponsePayload::Timeout => Err(BidirError::Timeout),
            other => Err(BidirError::TypeMismatch {
                expected: "Text".into(),
                got: format!("{:?}", other),
            }),
        }
    }

    /// Select from options (returns first selection or error)
    async fn select(
        &self,
        message: impl Into<String> + Send,
        options: Vec<SelectOption>,
    ) -> Result<String, BidirError> {
        match self
            .request(RequestType::Select {
                message: message.into(),
                options,
                multi_select: false,
            })
            .await?
        {
            ResponsePayload::Selected(mut v) => v.pop().ok_or(BidirError::Cancelled),
            ResponsePayload::Cancelled => Err(BidirError::Cancelled),
            ResponsePayload::Timeout => Err(BidirError::Timeout),
            other => Err(BidirError::TypeMismatch {
                expected: "Selected".into(),
                got: format!("{:?}", other),
            }),
        }
    }
}

// Blanket implementation
impl<T: BidirectionalContext> BidirExt for T {}

// ============================================================================
// Helper Functions and Patterns
// ============================================================================

/// Wrapper that provides fallback values when bidirectional not supported
pub struct BidirWithFallback<'a> {
    ctx: &'a dyn BidirectionalContext,
    auto_confirm: bool,
    default_text: Option<String>,
}

impl<'a> BidirWithFallback<'a> {
    pub fn new(ctx: &'a dyn BidirectionalContext) -> Self {
        Self {
            ctx,
            auto_confirm: false,
            default_text: None,
        }
    }

    pub fn auto_confirm(mut self) -> Self {
        self.auto_confirm = true;
        self
    }

    pub fn with_default(mut self, default: impl Into<String>) -> Self {
        self.default_text = Some(default.into());
        self
    }

    pub async fn confirm(&self, message: &str) -> bool {
        match self
            .ctx
            .request(RequestType::Confirm {
                message: message.to_string(),
                default: None,
            })
            .await
        {
            Ok(ResponsePayload::Confirmed(v)) => v,
            Err(BidirError::NotSupported) => self.auto_confirm,
            _ => false,
        }
    }

    pub async fn prompt(&self, message: &str) -> Option<String> {
        match self
            .ctx
            .request(RequestType::Prompt {
                message: message.to_string(),
                default: None,
                placeholder: None,
            })
            .await
        {
            Ok(ResponsePayload::Text(s)) => Some(s),
            Err(BidirError::NotSupported) => self.default_text.clone(),
            _ => None,
        }
    }
}

/// Configure timeouts for different request types
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    pub confirm: Duration,
    pub prompt: Duration,
    pub select: Duration,
    pub custom: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            confirm: Duration::from_secs(30),
            prompt: Duration::from_secs(60),
            select: Duration::from_secs(45),
            custom: Duration::from_secs(120),
        }
    }
}

impl TimeoutConfig {
    pub fn quick() -> Self {
        Self {
            confirm: Duration::from_secs(10),
            prompt: Duration::from_secs(30),
            select: Duration::from_secs(20),
            custom: Duration::from_secs(60),
        }
    }

    pub fn patient() -> Self {
        Self {
            confirm: Duration::from_secs(120),
            prompt: Duration::from_secs(300),
            select: Duration::from_secs(180),
            custom: Duration::from_secs(600),
        }
    }

    pub fn for_request_type(&self, request_type: &RequestType) -> Duration {
        match request_type {
            RequestType::Confirm { .. } => self.confirm,
            RequestType::Prompt { .. } => self.prompt,
            RequestType::Select { .. } => self.select,
            RequestType::Custom { .. } => self.custom,
        }
    }
}

/// Convert BidirError to a user-friendly message
pub fn bidir_error_message(error: &BidirError) -> String {
    match error {
        BidirError::Cancelled => "Operation cancelled by user".into(),
        BidirError::Timeout => "Timed out waiting for response".into(),
        BidirError::NotSupported => "Interactive mode not supported".into(),
        BidirError::TypeMismatch { expected, got } => {
            format!("Expected {} response, got {}", expected, got)
        }
        BidirError::Transport(msg) => format!("Communication error: {}", msg),
    }
}

// ============================================================================
// Channel-Based Activation Context (BIDIR-3)
// ============================================================================

/// Channel for bidirectional communication during method execution.
///
/// This provides the concrete implementation of `BidirectionalContext` that
/// activations use to send requests to clients and receive responses.
///
/// # Architecture
///
/// ```text
/// ┌─────────────────┐    stream_tx     ┌─────────────────┐
/// │   Activation    │ ───────────────► │   Transport     │
/// │  (BidirChannel) │                  │  (MCP/HTTP/WS)  │
/// │                 │ ◄─────────────── │                 │
/// └─────────────────┘  handle_response └─────────────────┘
/// ```
///
/// The channel maintains a map of pending requests, each with a oneshot channel
/// for receiving the response. When a response arrives via `handle_response`,
/// it's routed to the appropriate waiting task.
pub struct BidirChannel {
    /// Sender for PlexusStreamItem (including Request variants)
    stream_tx: mpsc::Sender<PlexusStreamItem>,
    /// Pending requests awaiting responses
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<ResponsePayload>>>>,
    /// Default timeout for requests
    default_timeout: Duration,
    /// Whether bidirectional is supported by transport
    bidirectional_supported: bool,
    /// Provenance path for metadata
    provenance: Vec<String>,
    /// Plexus hash for metadata
    plexus_hash: String,
}

impl BidirChannel {
    /// Create a new bidirectional channel.
    ///
    /// # Arguments
    /// * `stream_tx` - Channel for sending stream items to the transport
    /// * `bidirectional_supported` - Whether the transport supports bidirectional communication
    /// * `provenance` - The call path (e.g., ["plexus", "cone", "chat"])
    /// * `plexus_hash` - Current plexus configuration hash
    pub fn new(
        stream_tx: mpsc::Sender<PlexusStreamItem>,
        bidirectional_supported: bool,
        provenance: Vec<String>,
        plexus_hash: String,
    ) -> Self {
        Self {
            stream_tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            default_timeout: Duration::from_secs(30),
            bidirectional_supported,
            provenance,
            plexus_hash,
        }
    }

    /// Create a unidirectional-only channel (backward compatibility).
    ///
    /// Requests will return `BidirError::NotSupported` immediately.
    pub fn unidirectional(stream_tx: mpsc::Sender<PlexusStreamItem>) -> Self {
        Self::new(stream_tx, false, vec![], String::new())
    }

    /// Set the default timeout for requests
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Handle incoming response from client.
    ///
    /// This should be called by the transport layer when a `ClientResponse` is received.
    /// The response is routed to the task waiting on the corresponding request.
    ///
    /// # Errors
    /// Returns an error if there's no pending request with the given ID.
    pub fn handle_response(&self, response: ClientResponse) -> Result<(), BidirError> {
        let mut pending = self.pending.lock().map_err(|e| {
            BidirError::Transport(format!("Failed to acquire pending lock: {}", e))
        })?;

        if let Some(tx) = pending.remove(&response.request_id) {
            // If send fails, the receiver was dropped (timeout or cancellation)
            // This is not an error from the transport's perspective
            let _ = tx.send(response.payload);
            Ok(())
        } else {
            Err(BidirError::Transport(format!(
                "No pending request with ID: {}",
                response.request_id
            )))
        }
    }

    /// Get the number of pending requests
    pub fn pending_count(&self) -> usize {
        self.pending.lock().map(|p| p.len()).unwrap_or(0)
    }

    /// Create metadata for stream items
    fn create_metadata(&self) -> StreamMetadata {
        StreamMetadata::new(self.provenance.clone(), self.plexus_hash.clone())
    }

    fn generate_request_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

#[async_trait]
impl BidirectionalContext for BidirChannel {
    async fn request(&self, request_type: RequestType) -> Result<ResponsePayload, BidirError> {
        self.request_with_timeout(request_type, self.default_timeout)
            .await
    }

    async fn request_with_timeout(
        &self,
        request_type: RequestType,
        timeout: Duration,
    ) -> Result<ResponsePayload, BidirError> {
        if !self.bidirectional_supported {
            return Err(BidirError::NotSupported);
        }

        let request_id = Self::generate_request_id();
        let (tx, rx) = oneshot::channel();

        // Register the pending request
        {
            let mut pending = self.pending.lock().map_err(|e| {
                BidirError::Transport(format!("Failed to acquire pending lock: {}", e))
            })?;
            pending.insert(request_id.clone(), tx);
        }

        // Create and send the request item
        let request_item = PlexusStreamItem::request(
            self.create_metadata(),
            request_id.clone(),
            request_type,
            serde_json::Value::Null,
            Some(timeout.as_millis() as u64),
        );

        if let Err(_) = self.stream_tx.send(request_item).await {
            // Clean up the pending request
            self.pending.lock().ok().map(|mut p| p.remove(&request_id));
            return Err(BidirError::Transport("Stream channel closed".into()));
        }

        // Wait for response with timeout
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(payload)) => Ok(payload),
            Ok(Err(_)) => {
                // Receiver error - sender was dropped (likely due to handle_response cleanup)
                self.pending.lock().ok().map(|mut p| p.remove(&request_id));
                Err(BidirError::Transport("Response channel dropped".into()))
            }
            Err(_) => {
                // Timeout elapsed
                self.pending.lock().ok().map(|mut p| p.remove(&request_id));
                Err(BidirError::Timeout)
            }
        }
    }

    fn is_bidirectional(&self) -> bool {
        self.bidirectional_supported
    }
}

// Make BidirChannel cloneable for sharing across tasks
impl Clone for BidirChannel {
    fn clone(&self) -> Self {
        Self {
            stream_tx: self.stream_tx.clone(),
            pending: Arc::clone(&self.pending),
            default_timeout: self.default_timeout,
            bidirectional_supported: self.bidirectional_supported,
            provenance: self.provenance.clone(),
            plexus_hash: self.plexus_hash.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    // ========================================================================
    // Full Request-Response Cycle Tests
    // ========================================================================

    #[tokio::test]
    async fn test_bidir_channel_full_roundtrip_confirm() {
        let (tx, mut rx) = mpsc::channel(10);
        let channel = BidirChannel::new(
            tx,
            true,
            vec!["plexus".into(), "test".into()],
            "hash123".into(),
        );

        // Spawn the request in a task (it will wait for response)
        let channel_clone = channel.clone();
        let request_task = tokio::spawn(async move {
            channel_clone.confirm("Do you confirm?").await
        });

        // Receive the request from the stream
        let item = rx.recv().await.expect("Should receive stream item");

        if let PlexusStreamItem::Request {
            request_id,
            request_type,
            metadata,
            ..
        } = item
        {
            // Verify metadata
            assert_eq!(metadata.provenance, vec!["plexus", "test"]);
            assert_eq!(metadata.plexus_hash, "hash123");

            // Verify request type
            if let RequestType::Confirm { message, .. } = &request_type {
                assert_eq!(message, "Do you confirm?");
            } else {
                panic!("Expected Confirm request type, got {:?}", request_type);
            }

            // Send response
            channel
                .handle_response(ClientResponse {
                    request_id,
                    payload: ResponsePayload::Confirmed(true),
                })
                .expect("handle_response should succeed");
        } else {
            panic!("Expected Request variant, got {:?}", item);
        }

        // Verify the request task completes successfully with correct value
        let result = request_task.await.expect("Task should complete");
        assert_eq!(result.unwrap(), true);
    }

    #[tokio::test]
    async fn test_bidir_channel_full_roundtrip_prompt() {
        let (tx, mut rx) = mpsc::channel(10);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        let channel_clone = channel.clone();
        let request_task = tokio::spawn(async move {
            channel_clone.prompt("Enter your name").await
        });

        let item = rx.recv().await.expect("Should receive stream item");

        if let PlexusStreamItem::Request {
            request_id,
            request_type,
            ..
        } = item
        {
            if let RequestType::Prompt { message, .. } = &request_type {
                assert_eq!(message, "Enter your name");
            } else {
                panic!("Expected Prompt request type");
            }

            channel
                .handle_response(ClientResponse {
                    request_id,
                    payload: ResponsePayload::Text("Alice".into()),
                })
                .expect("handle_response should succeed");
        } else {
            panic!("Expected Request variant");
        }

        let result = request_task.await.expect("Task should complete");
        assert_eq!(result.unwrap(), "Alice");
    }

    #[tokio::test]
    async fn test_bidir_channel_full_roundtrip_select() {
        let (tx, mut rx) = mpsc::channel(10);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        let channel_clone = channel.clone();
        let options = vec![
            SelectOption {
                value: "opt1".into(),
                label: "Option 1".into(),
                description: None,
            },
            SelectOption {
                value: "opt2".into(),
                label: "Option 2".into(),
                description: Some("The second option".into()),
            },
        ];
        let request_task = tokio::spawn(async move {
            channel_clone.select("Choose one", options).await
        });

        let item = rx.recv().await.expect("Should receive stream item");

        if let PlexusStreamItem::Request {
            request_id,
            request_type,
            ..
        } = item
        {
            if let RequestType::Select { message, options, multi_select } = &request_type {
                assert_eq!(message, "Choose one");
                assert_eq!(options.len(), 2);
                assert_eq!(options[0].value, "opt1");
                assert_eq!(options[1].value, "opt2");
                assert!(!multi_select);
            } else {
                panic!("Expected Select request type");
            }

            channel
                .handle_response(ClientResponse {
                    request_id,
                    payload: ResponsePayload::Selected(vec!["opt2".into()]),
                })
                .expect("handle_response should succeed");
        } else {
            panic!("Expected Request variant");
        }

        let result = request_task.await.expect("Task should complete");
        assert_eq!(result.unwrap(), "opt2");
    }

    // ========================================================================
    // Error Handling Tests
    // ========================================================================

    #[tokio::test]
    async fn test_response_to_wrong_request_id_fails() {
        let (tx, mut rx) = mpsc::channel(10);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        let channel_clone = channel.clone();
        let request_task = tokio::spawn(async move {
            channel_clone
                .request_with_timeout(
                    RequestType::Confirm {
                        message: "test".into(),
                        default: None,
                    },
                    Duration::from_millis(100),
                )
                .await
        });

        // Wait for request to be sent
        let _item = rx.recv().await.expect("Should receive stream item");

        // Try to respond with wrong request_id
        let result = channel.handle_response(ClientResponse {
            request_id: "wrong-id-that-does-not-exist".into(),
            payload: ResponsePayload::Confirmed(true),
        });

        // Should fail because no pending request with this ID
        assert!(matches!(result, Err(BidirError::Transport(_))));
        if let Err(BidirError::Transport(msg)) = result {
            assert!(msg.contains("No pending request"));
        }

        // The original request should timeout since we didn't respond to it
        let task_result = request_task.await.expect("Task should complete");
        assert!(matches!(task_result, Err(BidirError::Timeout)));
    }

    #[tokio::test]
    async fn test_handle_response_unknown_id_returns_error() {
        let (tx, _rx) = mpsc::channel(1);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        let result = channel.handle_response(ClientResponse {
            request_id: "nonexistent-id".into(),
            payload: ResponsePayload::Confirmed(true),
        });

        assert!(matches!(result, Err(BidirError::Transport(_))));
    }

    #[tokio::test]
    async fn test_unidirectional_channel_request_fails() {
        let (tx, _rx) = mpsc::channel(1);
        let channel = BidirChannel::unidirectional(tx);

        assert!(!channel.is_bidirectional());

        let result = channel
            .request(RequestType::Confirm {
                message: "test".into(),
                default: None,
            })
            .await;

        assert!(matches!(result, Err(BidirError::NotSupported)));
    }

    #[tokio::test]
    async fn test_stream_closed_returns_transport_error() {
        let (tx, rx) = mpsc::channel(1);
        drop(rx); // Close the receiver

        let channel = BidirChannel::new(tx, true, vec![], String::new());

        let result = channel
            .request(RequestType::Confirm {
                message: "test".into(),
                default: None,
            })
            .await;

        assert!(matches!(result, Err(BidirError::Transport(_))));
        if let Err(BidirError::Transport(msg)) = result {
            assert!(msg.contains("closed") || msg.contains("channel"));
        }
    }

    #[tokio::test]
    async fn test_type_mismatch_on_wrong_response_type() {
        let (tx, mut rx) = mpsc::channel(10);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        let channel_clone = channel.clone();
        let request_task = tokio::spawn(async move {
            // Request confirm but will receive Text response
            channel_clone.confirm("Do you confirm?").await
        });

        let item = rx.recv().await.expect("Should receive stream item");

        if let PlexusStreamItem::Request { request_id, .. } = item {
            // Send wrong type of response (Text instead of Confirmed)
            channel
                .handle_response(ClientResponse {
                    request_id,
                    payload: ResponsePayload::Text("wrong type".into()),
                })
                .expect("handle_response should succeed");
        }

        let result = request_task.await.expect("Task should complete");
        assert!(matches!(result, Err(BidirError::TypeMismatch { .. })));
    }

    #[tokio::test]
    async fn test_cancelled_response() {
        let (tx, mut rx) = mpsc::channel(10);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        let channel_clone = channel.clone();
        let request_task = tokio::spawn(async move {
            channel_clone.confirm("Do you confirm?").await
        });

        let item = rx.recv().await.expect("Should receive stream item");

        if let PlexusStreamItem::Request { request_id, .. } = item {
            channel
                .handle_response(ClientResponse {
                    request_id,
                    payload: ResponsePayload::Cancelled,
                })
                .expect("handle_response should succeed");
        }

        let result = request_task.await.expect("Task should complete");
        assert!(matches!(result, Err(BidirError::Cancelled)));
    }

    // ========================================================================
    // Concurrent Request Tests
    // ========================================================================

    #[tokio::test]
    async fn test_multiple_concurrent_requests() {
        let (tx, mut rx) = mpsc::channel(10);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        // Spawn three concurrent requests
        let channel1 = channel.clone();
        let task1 = tokio::spawn(async move {
            channel1.confirm("Confirm 1?").await
        });

        let channel2 = channel.clone();
        let task2 = tokio::spawn(async move {
            channel2.prompt("Prompt 2?").await
        });

        let channel3 = channel.clone();
        let options = vec![SelectOption {
            value: "a".into(),
            label: "A".into(),
            description: None,
        }];
        let task3 = tokio::spawn(async move {
            channel3.select("Select 3?", options).await
        });

        // Give tasks time to send requests
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Collect all request IDs and respond to each
        let mut request_ids = Vec::new();
        for _ in 0..3 {
            if let Some(item) = rx.recv().await {
                if let PlexusStreamItem::Request {
                    request_id,
                    request_type,
                    ..
                } = item
                {
                    let payload = match request_type {
                        RequestType::Confirm { .. } => ResponsePayload::Confirmed(true),
                        RequestType::Prompt { .. } => ResponsePayload::Text("response".into()),
                        RequestType::Select { .. } => ResponsePayload::Selected(vec!["a".into()]),
                        _ => panic!("Unexpected request type"),
                    };
                    request_ids.push((request_id.clone(), payload));
                }
            }
        }

        // Respond to all requests
        for (request_id, payload) in request_ids {
            channel
                .handle_response(ClientResponse { request_id, payload })
                .expect("handle_response should succeed");
        }

        // Verify all tasks complete with correct values
        let result1 = task1.await.expect("Task 1 should complete");
        assert_eq!(result1.unwrap(), true);

        let result2 = task2.await.expect("Task 2 should complete");
        assert_eq!(result2.unwrap(), "response");

        let result3 = task3.await.expect("Task 3 should complete");
        assert_eq!(result3.unwrap(), "a");
    }

    #[tokio::test]
    async fn test_pending_count_tracks_active_requests() {
        let (tx, mut rx) = mpsc::channel(10);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        assert_eq!(channel.pending_count(), 0);

        // Start a request
        let channel_clone = channel.clone();
        let task = tokio::spawn(async move {
            channel_clone
                .request_with_timeout(
                    RequestType::Confirm {
                        message: "test".into(),
                        default: None,
                    },
                    Duration::from_millis(500),
                )
                .await
        });

        // Wait for request to be sent
        let item = rx.recv().await.expect("Should receive stream item");

        // Pending count should be 1
        assert_eq!(channel.pending_count(), 1);

        // Respond
        if let PlexusStreamItem::Request { request_id, .. } = item {
            channel
                .handle_response(ClientResponse {
                    request_id,
                    payload: ResponsePayload::Confirmed(true),
                })
                .expect("handle_response should succeed");
        }

        // Wait for task to complete
        let _ = task.await;

        // Pending count should be back to 0
        assert_eq!(channel.pending_count(), 0);
    }

    // ========================================================================
    // BidirWithFallback Tests
    // ========================================================================

    #[tokio::test]
    async fn test_fallback_confirm_returns_auto_confirm_value() {
        let (tx, _rx) = mpsc::channel(1);
        // Create unidirectional channel (bidirectional not supported)
        let channel = BidirChannel::unidirectional(tx);

        // Use BidirWithFallback with auto_confirm enabled
        let fallback = BidirWithFallback::new(&channel).auto_confirm();

        // Should return true without blocking (fallback value)
        let result = fallback.confirm("This would block without fallback").await;
        assert!(result);
    }

    #[tokio::test]
    async fn test_fallback_confirm_without_auto_confirm_returns_false() {
        let (tx, _rx) = mpsc::channel(1);
        let channel = BidirChannel::unidirectional(tx);

        // Fallback without auto_confirm
        let fallback = BidirWithFallback::new(&channel);

        // Should return false (default when not supported and no auto_confirm)
        let result = fallback.confirm("test").await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_fallback_prompt_returns_default_text() {
        let (tx, _rx) = mpsc::channel(1);
        let channel = BidirChannel::unidirectional(tx);

        let fallback = BidirWithFallback::new(&channel).with_default("default_value");

        // Should return the default text without blocking
        let result = fallback.prompt("Enter something").await;
        assert_eq!(result, Some("default_value".into()));
    }

    #[tokio::test]
    async fn test_fallback_prompt_without_default_returns_none() {
        let (tx, _rx) = mpsc::channel(1);
        let channel = BidirChannel::unidirectional(tx);

        let fallback = BidirWithFallback::new(&channel);

        // Should return None when not supported and no default
        let result = fallback.prompt("Enter something").await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_fallback_with_bidirectional_channel_works_normally() {
        let (tx, mut rx) = mpsc::channel(10);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        let channel_ref = &channel;
        let _fallback = BidirWithFallback::new(channel_ref).auto_confirm();

        // Spawn the confirm request
        let task = tokio::spawn({
            let channel = channel.clone();
            async move {
                let fallback = BidirWithFallback::new(&channel).auto_confirm();
                fallback.confirm("Real confirm?").await
            }
        });

        // Receive and respond
        let item = rx.recv().await.expect("Should receive stream item");
        if let PlexusStreamItem::Request { request_id, .. } = item {
            channel
                .handle_response(ClientResponse {
                    request_id,
                    payload: ResponsePayload::Confirmed(false),
                })
                .expect("handle_response should succeed");
        }

        // Should use real response, not fallback
        let result = task.await.expect("Task should complete");
        assert!(!result); // false from response, not true from fallback
    }

    // ========================================================================
    // Timeout Tests
    // ========================================================================

    #[tokio::test]
    async fn test_timeout_actually_expires() {
        let (tx, _rx) = mpsc::channel(10);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        let timeout_duration = Duration::from_millis(50);
        let start = Instant::now();

        let result = channel
            .request_with_timeout(
                RequestType::Confirm {
                    message: "test".into(),
                    default: None,
                },
                timeout_duration,
            )
            .await;

        let elapsed = start.elapsed();

        // Verify it actually took approximately the timeout duration
        assert!(matches!(result, Err(BidirError::Timeout)));
        assert!(
            elapsed >= timeout_duration,
            "Timeout returned too quickly: {:?} < {:?}",
            elapsed,
            timeout_duration
        );
        // Allow some margin for test execution overhead
        assert!(
            elapsed < timeout_duration + Duration::from_millis(50),
            "Timeout took too long: {:?}",
            elapsed
        );

        // Verify cleanup
        assert_eq!(channel.pending_count(), 0);
    }

    #[tokio::test]
    async fn test_timeout_cleanup_removes_pending_request() {
        let (tx, mut rx) = mpsc::channel(10);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        let channel_clone = channel.clone();
        let task = tokio::spawn(async move {
            channel_clone
                .request_with_timeout(
                    RequestType::Confirm {
                        message: "test".into(),
                        default: None,
                    },
                    Duration::from_millis(20),
                )
                .await
        });

        // Wait for request to be received
        let item = rx.recv().await.expect("Should receive stream item");
        let request_id = if let PlexusStreamItem::Request { request_id, .. } = item {
            request_id
        } else {
            panic!("Expected Request variant");
        };

        // Pending should be 1
        assert_eq!(channel.pending_count(), 1);

        // Wait for timeout
        let result = task.await.expect("Task should complete");
        assert!(matches!(result, Err(BidirError::Timeout)));

        // Pending should be cleaned up
        assert_eq!(channel.pending_count(), 0);

        // Responding after timeout should fail (request already removed)
        let late_response = channel.handle_response(ClientResponse {
            request_id,
            payload: ResponsePayload::Confirmed(true),
        });
        assert!(matches!(late_response, Err(BidirError::Transport(_))));
    }

    // ========================================================================
    // Clone and Sharing Tests
    // ========================================================================

    #[tokio::test]
    async fn test_cloned_channel_can_handle_response() {
        let (tx, mut rx) = mpsc::channel(10);
        let channel = BidirChannel::new(tx, true, vec![], String::new());
        let channel_clone = channel.clone();

        // Start request on original channel
        let task = tokio::spawn({
            let c = channel.clone();
            async move { c.confirm("test").await }
        });

        let item = rx.recv().await.expect("Should receive stream item");
        let request_id = if let PlexusStreamItem::Request { request_id, .. } = item {
            request_id
        } else {
            panic!("Expected Request variant");
        };

        // Handle response on cloned channel
        channel_clone
            .handle_response(ClientResponse {
                request_id,
                payload: ResponsePayload::Confirmed(true),
            })
            .expect("Cloned channel should be able to handle response");

        let result = task.await.expect("Task should complete");
        assert_eq!(result.unwrap(), true);
    }

    // ========================================================================
    // Edge Case Tests (BIDIR-9)
    // ========================================================================

    #[tokio::test]
    async fn test_rapid_sequential_requests_100() {
        let (tx, mut rx) = mpsc::channel(200);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        // Spawn responder task
        let channel_for_responder = channel.clone();
        let responder = tokio::spawn(async move {
            for i in 0..100 {
                if let Some(PlexusStreamItem::Request { request_id, .. }) = rx.recv().await {
                    channel_for_responder
                        .handle_response(ClientResponse {
                            request_id,
                            payload: ResponsePayload::Text(format!("response-{}", i)),
                        })
                        .expect("handle_response should succeed");
                }
            }
        });

        // Run 100 sequential requests
        for i in 0..100 {
            let result = channel
                .prompt(&format!("Question {}", i))
                .await
                .expect("Request should succeed");
            assert_eq!(result, format!("response-{}", i));
        }

        responder.await.expect("Responder should complete");
        assert_eq!(channel.pending_count(), 0, "All requests should be cleaned up");
    }

    #[tokio::test]
    async fn test_empty_selection_list_returns_error() {
        let (tx, mut rx) = mpsc::channel(10);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        let channel_clone = channel.clone();
        let request_task = tokio::spawn(async move {
            // Pass empty options list
            channel_clone.select("Choose one", vec![]).await
        });

        // Wait for request
        let item = rx.recv().await.expect("Should receive stream item");

        if let PlexusStreamItem::Request {
            request_id,
            request_type,
            ..
        } = item
        {
            // Verify the select request was sent with empty options
            if let RequestType::Select { options, .. } = &request_type {
                assert!(options.is_empty(), "Options should be empty");
            }

            // Respond with empty selection (as would happen with no options)
            channel
                .handle_response(ClientResponse {
                    request_id,
                    payload: ResponsePayload::Selected(vec![]),
                })
                .expect("handle_response should succeed");
        }

        // Should return Cancelled since pop() on empty vec returns None
        let result = request_task.await.expect("Task should complete");
        assert!(
            matches!(result, Err(BidirError::Cancelled)),
            "Empty selection should return Cancelled, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_very_long_text_in_prompt_response() {
        let (tx, mut rx) = mpsc::channel(10);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        // Create a 1MB string
        let long_text: String = "x".repeat(1_000_000);
        let expected_text = long_text.clone();

        let channel_clone = channel.clone();
        let request_task = tokio::spawn(async move {
            channel_clone.prompt("Enter a very long response").await
        });

        let item = rx.recv().await.expect("Should receive stream item");

        if let PlexusStreamItem::Request { request_id, .. } = item {
            channel
                .handle_response(ClientResponse {
                    request_id,
                    payload: ResponsePayload::Text(long_text),
                })
                .expect("handle_response should succeed");
        }

        let result = request_task.await.expect("Task should complete");
        let response = result.expect("Should get response");
        assert_eq!(
            response.len(),
            1_000_000,
            "Should handle 1MB text response"
        );
        assert_eq!(response, expected_text);
    }

    #[tokio::test]
    async fn test_unicode_in_messages_and_responses() {
        let (tx, mut rx) = mpsc::channel(10);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        // Unicode test cases: emoji, CJK, Arabic, combining characters
        let unicode_message = "Hello \u{1F600} \u{4E2D}\u{6587} \u{0627}\u{0644}\u{0639}\u{0631}\u{0628}\u{064A}\u{0629} e\u{0301}";
        let unicode_response = "\u{1F389}\u{1F38A} \u{65E5}\u{672C}\u{8A9E} \u{05E2}\u{05D1}\u{05E8}\u{05D9}\u{05EA}";

        let channel_clone = channel.clone();
        let expected_response = unicode_response.to_string();
        let request_task = tokio::spawn(async move {
            channel_clone.prompt(unicode_message).await
        });

        let item = rx.recv().await.expect("Should receive stream item");

        if let PlexusStreamItem::Request {
            request_id,
            request_type,
            ..
        } = item
        {
            // Verify unicode was preserved in request
            if let RequestType::Prompt { message, .. } = &request_type {
                assert_eq!(message, unicode_message, "Unicode should be preserved in request");
            }

            channel
                .handle_response(ClientResponse {
                    request_id,
                    payload: ResponsePayload::Text(unicode_response.to_string()),
                })
                .expect("handle_response should succeed");
        }

        let result = request_task.await.expect("Task should complete");
        let response = result.expect("Should get response");
        assert_eq!(response, expected_response, "Unicode should be preserved in response");
    }

    #[tokio::test]
    async fn test_concurrent_same_message_gets_unique_ids() {
        let (tx, mut rx) = mpsc::channel(50);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        let identical_message = "Same message for all";

        // Spawn 10 concurrent requests with identical message
        let mut tasks = Vec::new();
        for _ in 0..10 {
            let c = channel.clone();
            let msg = identical_message.to_string();
            tasks.push(tokio::spawn(async move {
                c.request_with_timeout(
                    RequestType::Prompt {
                        message: msg,
                        default: None,
                        placeholder: None,
                    },
                    Duration::from_secs(5),
                )
                .await
            }));
        }

        // Give tasks time to send requests
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Collect all request IDs
        let mut request_ids = Vec::new();
        for _ in 0..10 {
            if let Some(PlexusStreamItem::Request {
                request_id,
                request_type,
                ..
            }) = rx.recv().await
            {
                // Verify message is the same
                if let RequestType::Prompt { message, .. } = &request_type {
                    assert_eq!(message, identical_message);
                }
                request_ids.push(request_id);
            }
        }

        // All IDs should be unique
        let unique_ids: std::collections::HashSet<_> = request_ids.iter().collect();
        assert_eq!(
            unique_ids.len(),
            10,
            "All 10 concurrent requests should have unique IDs"
        );

        // Respond to all
        for (i, request_id) in request_ids.iter().enumerate() {
            channel
                .handle_response(ClientResponse {
                    request_id: request_id.clone(),
                    payload: ResponsePayload::Text(format!("response-{}", i)),
                })
                .expect("handle_response should succeed");
        }

        // Verify all tasks complete
        for task in tasks {
            let result = task.await.expect("Task should complete");
            assert!(result.is_ok(), "Request should succeed");
        }
    }

    #[tokio::test]
    async fn test_stream_closed_mid_request_during_wait() {
        let (tx, rx) = mpsc::channel(10);
        let channel = BidirChannel::new(tx, true, vec![], String::new());

        let channel_clone = channel.clone();
        let request_task = tokio::spawn(async move {
            channel_clone
                .request_with_timeout(
                    RequestType::Confirm {
                        message: "test".into(),
                        default: None,
                    },
                    Duration::from_secs(5),
                )
                .await
        });

        // Wait a bit for request to be sent and in waiting state
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Drop the receiver, simulating stream closure
        drop(rx);

        // Pending count should still be 1 (waiting for response)
        assert_eq!(channel.pending_count(), 1);

        // The request should eventually timeout since no response will come
        // (the stream_tx.send already succeeded before we dropped rx)
        let result = tokio::time::timeout(Duration::from_millis(100), request_task).await;

        // Either timeout from our test wrapper or the actual request timeout
        match result {
            Ok(Ok(Err(BidirError::Timeout))) => {
                // Expected - request timed out waiting for response
            }
            Ok(Ok(Err(BidirError::Transport(_)))) => {
                // Also acceptable - transport error
            }
            Ok(Err(_)) => {
                // JoinError - task panicked, not expected
                panic!("Task should not panic");
            }
            Err(_) => {
                // Our test timeout - request is still waiting, that's fine
                // The internal timeout would eventually fire
            }
            other => panic!("Unexpected result: {:?}", other),
        }
    }
}
