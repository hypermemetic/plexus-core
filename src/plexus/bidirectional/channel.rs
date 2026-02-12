//! Generic bidirectional channel implementation
//!
//! Provides type-safe server-to-client requests during streaming execution.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use uuid::Uuid;

use super::registry::{register_pending_request, unregister_pending_request};
use super::types::{BidirError, SelectOption, StandardRequest, StandardResponse};
use crate::plexus::types::PlexusStreamItem;

/// Generic bidirectional channel for type-safe server→client requests
///
/// This is the core primitive for bidirectional communication. Activations
/// use this to request input from clients during stream execution.
///
/// # Type Parameters
///
/// * `Req` - The request type sent from server to client (must be serializable)
/// * `Resp` - The response type sent from client to server (must be deserializable)
///
/// # Examples
///
/// ## Using StandardBidirChannel (type alias)
///
/// ```rust,ignore
/// use plexus_core::bidirectional::{StandardBidirChannel, BidirError};
///
/// async fn my_activation(ctx: &StandardBidirChannel) -> Result<(), BidirError> {
///     // Simple confirmation
///     if ctx.confirm("Delete this file?").await? {
///         // proceed
///     }
///
///     // Text input
///     let name = ctx.prompt("Enter your name:").await?;
///
///     Ok(())
/// }
/// ```
///
/// ## Using custom request/response types
///
/// ```rust,ignore
/// #[derive(Serialize, Deserialize)]
/// enum MyRequest { Ask { question: String } }
///
/// #[derive(Serialize, Deserialize)]
/// enum MyResponse { Answer { text: String } }
///
/// async fn my_activation(ctx: &BidirChannel<MyRequest, MyResponse>) -> Result<(), BidirError> {
///     let resp = ctx.request(MyRequest::Ask { question: "What's your favorite color?" }).await?;
///     match resp {
///         MyResponse::Answer { text } => println!("You said: {}", text),
///     }
///     Ok(())
/// }
/// ```
pub struct BidirChannel<Req, Resp>
where
    Req: Serialize + DeserializeOwned + Send + 'static,
    Resp: Serialize + DeserializeOwned + Send + 'static,
{
    /// Channel to send PlexusStreamItems (including Request items)
    stream_tx: mpsc::Sender<PlexusStreamItem>,

    /// Pending requests waiting for responses
    /// Maps request_id -> oneshot channel for response
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Resp>>>>,

    /// Whether bidirectional communication is supported by transport
    bidirectional_supported: bool,

    /// Whether to use global registry for response routing (for MCP transport)
    /// When true, responses come through the global registry instead of handle_response()
    use_global_registry: bool,

    /// Provenance path (for debugging/logging)
    provenance: Vec<String>,

    /// Plexus hash (for metadata)
    plexus_hash: String,

    /// Phantom data to hold Req type parameter
    _phantom_req: PhantomData<Req>,
}

/// Type alias for standard interactive UI patterns
pub type StandardBidirChannel = BidirChannel<StandardRequest, StandardResponse>;

impl<Req, Resp> BidirChannel<Req, Resp>
where
    Req: Serialize + DeserializeOwned + Send + 'static,
    Resp: Serialize + DeserializeOwned + Send + 'static,
{
    /// Create a new bidirectional channel
    ///
    /// By default, uses the global response registry which works with all transport types:
    /// - MCP: Responses come through `_plexus_respond` tool → global registry
    /// - WebSocket: Responses can also use global registry via `handle_pending_response()`
    ///
    /// Use `new_direct()` if you need direct response handling (for testing or specific transports).
    pub fn new(
        stream_tx: mpsc::Sender<PlexusStreamItem>,
        bidirectional_supported: bool,
        provenance: Vec<String>,
        plexus_hash: String,
    ) -> Self {
        Self {
            stream_tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            bidirectional_supported,
            use_global_registry: true, // Use global registry by default for transport compatibility
            provenance,
            plexus_hash,
            _phantom_req: PhantomData,
        }
    }

    /// Create a bidirectional channel that uses direct response handling
    ///
    /// Responses must be delivered via `handle_response()` method on this channel instance.
    /// Use this for testing or when you have direct access to the channel for responses.
    pub fn new_direct(
        stream_tx: mpsc::Sender<PlexusStreamItem>,
        bidirectional_supported: bool,
        provenance: Vec<String>,
        plexus_hash: String,
    ) -> Self {
        Self {
            stream_tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            bidirectional_supported,
            use_global_registry: false,
            provenance,
            plexus_hash,
            _phantom_req: PhantomData,
        }
    }

    /// Check if bidirectional communication is supported
    pub fn is_bidirectional(&self) -> bool {
        self.bidirectional_supported
    }

    /// Make a bidirectional request with default timeout (30s)
    ///
    /// Sends a request to the client and waits for response.
    /// Returns error if transport doesn't support bidirectional or timeout occurs.
    pub async fn request(&self, req: Req) -> Result<Resp, BidirError> {
        self.request_with_timeout(req, Duration::from_secs(30))
            .await
    }

    /// Make a bidirectional request with custom timeout
    pub async fn request_with_timeout(
        &self,
        req: Req,
        timeout_duration: Duration,
    ) -> Result<Resp, BidirError> {
        if !self.bidirectional_supported {
            return Err(BidirError::NotSupported);
        }

        // Generate unique request ID
        let request_id = Uuid::new_v4().to_string();

        // Serialize request
        let request_data = serde_json::to_value(&req)
            .map_err(|e| BidirError::Serialization(e.to_string()))?;

        let timeout_ms = timeout_duration.as_millis() as u64;

        if self.use_global_registry {
            // Use global registry for response routing (MCP transport)
            self.request_via_registry(request_id, request_data, timeout_duration, timeout_ms)
                .await
        } else {
            // Use internal pending map (WebSocket/direct transport)
            self.request_direct(request_id, request_data, timeout_duration, timeout_ms)
                .await
        }
    }

    /// Request using internal pending map (for direct transports like WebSocket)
    async fn request_direct(
        &self,
        request_id: String,
        request_data: Value,
        timeout_duration: Duration,
        timeout_ms: u64,
    ) -> Result<Resp, BidirError> {
        // Create oneshot channel for response
        let (tx, rx) = oneshot::channel();

        // Register pending request in internal map
        self.pending.lock().unwrap().insert(request_id.clone(), tx);

        // Send Request stream item
        self.stream_tx
            .send(PlexusStreamItem::request(
                request_id.clone(),
                request_data,
                timeout_ms,
            ))
            .await
            .map_err(|e| BidirError::Transport(format!("Failed to send request: {}", e)))?;

        // Wait for response (or timeout)
        match timeout(timeout_duration, rx).await {
            Ok(Ok(resp)) => Ok(resp),
            Ok(Err(_)) => {
                // Channel closed before response
                self.pending.lock().unwrap().remove(&request_id);
                Err(BidirError::ChannelClosed)
            }
            Err(_) => {
                // Timeout
                self.pending.lock().unwrap().remove(&request_id);
                Err(BidirError::Timeout(timeout_ms))
            }
        }
    }

    /// Request using global registry (for MCP transport via _plexus_respond tool)
    async fn request_via_registry(
        &self,
        request_id: String,
        request_data: Value,
        timeout_duration: Duration,
        timeout_ms: u64,
    ) -> Result<Resp, BidirError> {
        // Create oneshot channel for Value response (type-erased)
        let (tx, rx) = oneshot::channel::<Value>();

        // Register in global registry
        register_pending_request(request_id.clone(), tx);

        // Send Request stream item
        if let Err(e) = self
            .stream_tx
            .send(PlexusStreamItem::request(
                request_id.clone(),
                request_data,
                timeout_ms,
            ))
            .await
        {
            // Clean up on failure
            unregister_pending_request(&request_id);
            return Err(BidirError::Transport(format!("Failed to send request: {}", e)));
        }

        // Wait for response (or timeout)
        match timeout(timeout_duration, rx).await {
            Ok(Ok(value)) => {
                // Deserialize Value to typed response
                serde_json::from_value(value).map_err(|e| BidirError::TypeMismatch {
                    expected: std::any::type_name::<Resp>().to_string(),
                    got: e.to_string(),
                })
            }
            Ok(Err(_)) => {
                // Channel closed before response
                unregister_pending_request(&request_id);
                Err(BidirError::ChannelClosed)
            }
            Err(_) => {
                // Timeout - clean up from registry
                unregister_pending_request(&request_id);
                Err(BidirError::Timeout(timeout_ms))
            }
        }
    }

    /// Handle a response from the client
    ///
    /// Called by transport layer when client responds to a request.
    /// Deserializes response and sends it through the pending request's channel.
    pub fn handle_response(
        &self,
        request_id: String,
        response_data: Value,
    ) -> Result<(), BidirError> {
        // Look up pending request
        let tx = self
            .pending
            .lock()
            .unwrap()
            .remove(&request_id)
            .ok_or(BidirError::UnknownRequest)?;

        // Deserialize response
        let resp: Resp = serde_json::from_value(response_data).map_err(|e| {
            BidirError::TypeMismatch {
                expected: std::any::type_name::<Resp>().to_string(),
                got: e.to_string(),
            }
        })?;

        // Send response through channel (unblocks request() call)
        tx.send(resp).map_err(|_| BidirError::ChannelClosed)?;

        Ok(())
    }

    /// Get provenance path (for debugging)
    pub fn provenance(&self) -> &[String] {
        &self.provenance
    }

    /// Get plexus hash (for metadata)
    pub fn plexus_hash(&self) -> &str {
        &self.plexus_hash
    }
}

// Convenience methods for StandardBidirChannel
impl BidirChannel<StandardRequest, StandardResponse> {
    /// Ask user for yes/no confirmation
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if ctx.confirm("Delete this file?").await? {
    ///     // user confirmed
    /// }
    /// ```
    pub async fn confirm(&self, message: &str) -> Result<bool, BidirError> {
        let resp = self
            .request(StandardRequest::Confirm {
                message: message.to_string(),
                default: None,
            })
            .await?;

        match resp {
            StandardResponse::Confirmed(b) => Ok(b),
            StandardResponse::Cancelled => Err(BidirError::Cancelled),
            _ => Err(BidirError::TypeMismatch {
                expected: "Confirmed".into(),
                got: format!("{:?}", resp),
            }),
        }
    }

    /// Ask user for text input
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let name = ctx.prompt("Enter your name:").await?;
    /// ```
    pub async fn prompt(&self, message: &str) -> Result<String, BidirError> {
        let resp = self
            .request(StandardRequest::Prompt {
                message: message.to_string(),
                default: None,
                placeholder: None,
            })
            .await?;

        match resp {
            StandardResponse::Text(s) => Ok(s),
            StandardResponse::Cancelled => Err(BidirError::Cancelled),
            _ => Err(BidirError::TypeMismatch {
                expected: "Text".into(),
                got: format!("{:?}", resp),
            }),
        }
    }

    /// Ask user to select from options
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let options = vec![
    ///     SelectOption::new("dev", "Development"),
    ///     SelectOption::new("prod", "Production"),
    /// ];
    /// let selected = ctx.select("Choose environment:", options).await?;
    /// ```
    pub async fn select(
        &self,
        message: &str,
        options: Vec<SelectOption>,
    ) -> Result<Vec<String>, BidirError> {
        let resp = self
            .request(StandardRequest::Select {
                message: message.to_string(),
                options,
                multi_select: false,
            })
            .await?;

        match resp {
            StandardResponse::Selected(s) => Ok(s),
            StandardResponse::Cancelled => Err(BidirError::Cancelled),
            _ => Err(BidirError::TypeMismatch {
                expected: "Selected".into(),
                got: format!("{:?}", resp),
            }),
        }
    }
}

/// Bidirectional channel with fallback when transport doesn't support bidirectional
///
/// Wraps a BidirChannel and provides fallback values when bidirectional
/// requests fail due to NotSupported error.
pub struct BidirWithFallback<Req, Resp>
where
    Req: Serialize + DeserializeOwned + Send + 'static,
    Resp: Serialize + DeserializeOwned + Send + 'static,
{
    channel: Arc<BidirChannel<Req, Resp>>,
    fallback_fn: Box<dyn Fn(&Req) -> Resp + Send + Sync>,
}

impl<Req, Resp> BidirWithFallback<Req, Resp>
where
    Req: Serialize + DeserializeOwned + Send + 'static,
    Resp: Serialize + DeserializeOwned + Send + 'static,
{
    /// Create a new fallback wrapper with custom fallback function
    pub fn new(
        channel: Arc<BidirChannel<Req, Resp>>,
        fallback: impl Fn(&Req) -> Resp + Send + Sync + 'static,
    ) -> Self {
        Self {
            channel,
            fallback_fn: Box::new(fallback),
        }
    }

    /// Make a request, using fallback if bidirectional not supported
    pub async fn request(&self, req: Req) -> Resp
    where
        Req: Clone,
    {
        match self.channel.request(req.clone()).await {
            Ok(resp) => resp,
            Err(BidirError::NotSupported) | Err(BidirError::Timeout(_)) => {
                (self.fallback_fn)(&req)
            }
            Err(_) => (self.fallback_fn)(&req),
        }
    }
}

// Helper for StandardBidirChannel fallbacks
impl BidirWithFallback<StandardRequest, StandardResponse> {
    /// Create fallback that auto-confirms all requests
    pub fn auto_confirm(
        channel: Arc<BidirChannel<StandardRequest, StandardResponse>>,
    ) -> Self {
        Self::new(channel, |req| match req {
            StandardRequest::Confirm { default, .. } => {
                StandardResponse::Confirmed(default.unwrap_or(true))
            }
            StandardRequest::Prompt { default, .. } => {
                StandardResponse::Text(default.clone().unwrap_or_default())
            }
            StandardRequest::Select { options, .. } => StandardResponse::Selected(vec![options
                .first()
                .map(|o| o.value.clone())
                .unwrap_or_default()]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bidir_channel_not_supported() {
        let (tx, _rx) = mpsc::channel(32);
        let channel: BidirChannel<StandardRequest, StandardResponse> =
            BidirChannel::new_direct(tx, false, vec!["test".into()], "hash".into());

        let result = channel.confirm("Test?").await;
        assert!(matches!(result, Err(BidirError::NotSupported)));
    }

    #[tokio::test]
    async fn test_bidir_request_response() {
        let (tx, mut rx) = mpsc::channel(32);
        let channel: Arc<BidirChannel<StandardRequest, StandardResponse>> = Arc::new(BidirChannel::new_direct(
            tx,
            true,
            vec!["test".into()],
            "hash".into(),
        ));

        // Spawn request in background
        let channel_clone = channel.clone();
        let handle = tokio::spawn(async move {
            channel_clone
                .request(StandardRequest::Confirm {
                    message: "Test?".into(),
                    default: None,
                })
                .await
        });

        // Receive request
        if let Some(PlexusStreamItem::Request {
            request_id,
            request_data,
            ..
        }) = rx.recv().await
        {
            // Verify request
            let req: StandardRequest = serde_json::from_value(request_data).unwrap();
            assert!(matches!(req, StandardRequest::Confirm { .. }));

            // Send response
            channel
                .handle_response(
                    request_id,
                    serde_json::to_value(&StandardResponse::Confirmed(true)).unwrap(),
                )
                .unwrap();
        } else {
            panic!("Expected Request item");
        }

        // Verify response received
        let result: StandardResponse = handle.await.unwrap().unwrap();
        assert_eq!(result, StandardResponse::Confirmed(true));
    }

    #[tokio::test]
    async fn test_convenience_methods() {
        let (tx, mut rx) = mpsc::channel(32);
        let channel: Arc<StandardBidirChannel> = Arc::new(BidirChannel::new_direct(
            tx,
            true,
            vec!["test".into()],
            "hash".into(),
        ));

        // Test confirm()
        let channel_clone = channel.clone();
        let handle = tokio::spawn(async move { channel_clone.confirm("Delete?").await });

        if let Some(PlexusStreamItem::Request { request_id, .. }) = rx.recv().await {
            channel
                .handle_response(
                    request_id,
                    serde_json::to_value(&StandardResponse::Confirmed(true)).unwrap(),
                )
                .unwrap();
        }

        assert_eq!(handle.await.unwrap().unwrap(), true);
    }

    #[tokio::test]
    async fn test_timeout() {
        let (tx, _rx) = mpsc::channel(32);
        let channel: BidirChannel<StandardRequest, StandardResponse> =
            BidirChannel::new_direct(tx, true, vec!["test".into()], "hash".into());

        let result = channel
            .request_with_timeout(
                StandardRequest::Confirm {
                    message: "Test?".into(),
                    default: None,
                },
                Duration::from_millis(100),
            )
            .await;

        assert!(matches!(result, Err(BidirError::Timeout(100))));
    }

    #[tokio::test]
    async fn test_fallback() {
        let (tx, _rx) = mpsc::channel(32);
        let channel = Arc::new(BidirChannel::new_direct(
            tx,
            false, // not supported
            vec!["test".into()],
            "hash".into(),
        ));

        let fallback = BidirWithFallback::auto_confirm(channel);

        let resp = fallback
            .request(StandardRequest::Confirm {
                message: "Test?".into(),
                default: Some(false),
            })
            .await;

        assert_eq!(resp, StandardResponse::Confirmed(false));
    }
}
