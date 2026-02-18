//! Unit tests for bidirectional communication primitives
//!
//! These tests verify the core bidirectional channel functionality:
//! - BidirChannel request/response flow
//! - Global registry behavior (register, unregister, handle_pending_response)
//! - Timeout scenarios
//! - Error handling

use plexus_core::plexus::bidirectional::{
    auto_confirm_channel, auto_respond_channel, bidir_error_message, create_test_bidir_channel,
    create_test_standard_channel, handle_pending_response, is_request_pending, pending_count,
    register_pending_request, unregister_pending_request, BidirChannel, BidirError,
    BidirWithFallback, SelectOption, StandardBidirChannel, StandardRequest, StandardResponse,
    TimeoutConfig,
};
use plexus_core::plexus::types::PlexusStreamItem;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

// =============================================================================
// BidirChannel Basic Tests
// =============================================================================

#[tokio::test]
async fn test_bidir_channel_creation() {
    let (tx, _rx) = mpsc::channel(32);
    let channel: StandardBidirChannel =
        BidirChannel::new_direct(tx, true, vec!["test".into()], "hash123".into());

    assert!(channel.is_bidirectional());
    assert_eq!(channel.provenance(), &["test"]);
    assert_eq!(channel.plexus_hash(), "hash123");
}

#[tokio::test]
async fn test_bidir_channel_not_supported() {
    let (tx, _rx) = mpsc::channel(32);
    let channel: StandardBidirChannel =
        BidirChannel::new_direct(tx, false, vec![], "hash".into());

    assert!(!channel.is_bidirectional());

    let result = channel.confirm("Test?").await;
    assert!(matches!(result, Err(BidirError::NotSupported)));
}

#[tokio::test]
async fn test_bidir_channel_direct_flow() {
    let (tx, mut rx) = mpsc::channel(32);
    let channel: Arc<StandardBidirChannel> =
        Arc::new(BidirChannel::new_direct(tx, true, vec![], "hash".into()));

    let channel_clone = channel.clone();
    let handle = tokio::spawn(async move { channel_clone.confirm("Delete?").await });

    // Process request
    if let Some(PlexusStreamItem::Request {
        request_id,
        request_data,
        timeout_ms,
    }) = rx.recv().await
    {
        // Verify request structure
        assert!(!request_id.is_empty());
        assert_eq!(timeout_ms, 30000); // Default timeout

        let req: StandardRequest = serde_json::from_value(request_data).unwrap();
        assert!(matches!(req, StandardRequest::Confirm { message, .. } if message == "Delete?"));

        // Send response
        channel
            .handle_response(
                request_id,
                serde_json::to_value(&StandardResponse::<serde_json::Value>::Confirmed {
                    value: true,
                })
                .unwrap(),
            )
            .unwrap();
    }

    let result = handle.await.unwrap().unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_bidir_channel_prompt_flow() {
    let (tx, mut rx) = mpsc::channel(32);
    let channel: Arc<StandardBidirChannel> =
        Arc::new(BidirChannel::new_direct(tx, true, vec![], "hash".into()));

    let channel_clone = channel.clone();
    let handle = tokio::spawn(async move { channel_clone.prompt("Name?").await });

    if let Some(PlexusStreamItem::Request { request_id, .. }) = rx.recv().await {
        channel
            .handle_response(
                request_id,
                serde_json::to_value(&StandardResponse::<serde_json::Value>::Text {
                    value: serde_json::Value::String("Alice".into()),
                })
                .unwrap(),
            )
            .unwrap();
    }

    assert_eq!(handle.await.unwrap().unwrap(), "Alice");
}

#[tokio::test]
async fn test_bidir_channel_select_flow() {
    let (tx, mut rx) = mpsc::channel(32);
    let channel: Arc<StandardBidirChannel> =
        Arc::new(BidirChannel::new_direct(tx, true, vec![], "hash".into()));

    let channel_clone = channel.clone();
    let handle = tokio::spawn(async move {
        let options = vec![
            SelectOption::new("a", "Option A"),
            SelectOption::new("b", "Option B"),
        ];
        channel_clone.select("Choose:", options).await
    });

    if let Some(PlexusStreamItem::Request { request_id, .. }) = rx.recv().await {
        channel
            .handle_response(
                request_id,
                serde_json::to_value(&StandardResponse::<serde_json::Value>::Selected {
                    values: vec![serde_json::Value::String("b".into())],
                })
                .unwrap(),
            )
            .unwrap();
    }

    assert_eq!(handle.await.unwrap().unwrap(), vec!["b".to_string()]);
}

#[tokio::test]
async fn test_bidir_channel_cancelled_response() {
    let (tx, mut rx) = mpsc::channel(32);
    let channel: Arc<StandardBidirChannel> =
        Arc::new(BidirChannel::new_direct(tx, true, vec![], "hash".into()));

    let channel_clone = channel.clone();
    let handle = tokio::spawn(async move { channel_clone.confirm("Test?").await });

    if let Some(PlexusStreamItem::Request { request_id, .. }) = rx.recv().await {
        channel
            .handle_response(
                request_id,
                serde_json::to_value(&StandardResponse::<serde_json::Value>::Cancelled).unwrap(),
            )
            .unwrap();
    }

    let result = handle.await.unwrap();
    assert!(matches!(result, Err(BidirError::Cancelled)));
}

// =============================================================================
// Timeout Tests
// =============================================================================

#[tokio::test]
async fn test_bidir_channel_timeout() {
    let (tx, _rx) = mpsc::channel(32);
    let channel: StandardBidirChannel =
        BidirChannel::new_direct(tx, true, vec![], "hash".into());

    let start = std::time::Instant::now();
    let result = channel
        .request_with_timeout(
            StandardRequest::Confirm {
                message: "Test?".into(),
                default: None,
            },
            Duration::from_millis(50),
        )
        .await;

    let elapsed = start.elapsed();
    assert!(matches!(result, Err(BidirError::Timeout(50))));
    assert!(elapsed >= Duration::from_millis(45));
    assert!(elapsed < Duration::from_millis(150));
}

#[tokio::test]
async fn test_bidir_channel_custom_timeout() {
    let (tx, _rx) = mpsc::channel(32);
    let channel: StandardBidirChannel =
        BidirChannel::new_direct(tx, true, vec![], "hash".into());

    let result = channel
        .request_with_timeout(
            StandardRequest::Prompt {
                message: "?".into(),
                default: None,
                placeholder: None,
            },
            Duration::from_millis(25),
        )
        .await;

    assert!(matches!(result, Err(BidirError::Timeout(25))));
}

// =============================================================================
// Global Registry Tests
// =============================================================================

#[tokio::test]
async fn test_registry_register_and_handle() {
    let (tx, rx) = oneshot::channel::<Value>();
    let request_id = format!("test-{}", uuid::Uuid::new_v4());

    // Register
    register_pending_request(request_id.clone(), tx);
    assert!(is_request_pending(&request_id));

    // Handle response
    let response = serde_json::json!({"status": "ok"});
    handle_pending_response(&request_id, response.clone()).unwrap();

    // Verify response received
    let received = rx.await.unwrap();
    assert_eq!(received, response);

    // Should be removed
    assert!(!is_request_pending(&request_id));
}

#[tokio::test]
async fn test_registry_unknown_request() {
    let result = handle_pending_response("unknown-request-id", serde_json::json!({}));
    assert!(matches!(result, Err(BidirError::UnknownRequest)));
}

#[tokio::test]
async fn test_registry_unregister() {
    let (tx, _rx) = oneshot::channel::<Value>();
    let request_id = format!("test-unregister-{}", uuid::Uuid::new_v4());

    register_pending_request(request_id.clone(), tx);
    assert!(is_request_pending(&request_id));

    let removed = unregister_pending_request(&request_id);
    assert!(removed.is_some());
    assert!(!is_request_pending(&request_id));

    // Double unregister returns None
    let removed_again = unregister_pending_request(&request_id);
    assert!(removed_again.is_none());
}

#[tokio::test]
async fn test_registry_channel_closed() {
    let (tx, rx) = oneshot::channel::<Value>();
    let request_id = format!("test-closed-{}", uuid::Uuid::new_v4());

    register_pending_request(request_id.clone(), tx);

    // Drop receiver
    drop(rx);

    // Handle should fail with ChannelClosed
    let result = handle_pending_response(&request_id, serde_json::json!({}));
    assert!(matches!(result, Err(BidirError::ChannelClosed)));
}

#[tokio::test]
async fn test_registry_pending_count() {
    let initial_count = pending_count();

    let (tx1, _rx1) = oneshot::channel::<Value>();
    let (tx2, _rx2) = oneshot::channel::<Value>();
    let id1 = format!("count-test-1-{}", uuid::Uuid::new_v4());
    let id2 = format!("count-test-2-{}", uuid::Uuid::new_v4());

    register_pending_request(id1.clone(), tx1);
    assert_eq!(pending_count(), initial_count + 1);

    register_pending_request(id2.clone(), tx2);
    assert_eq!(pending_count(), initial_count + 2);

    unregister_pending_request(&id1);
    assert_eq!(pending_count(), initial_count + 1);

    unregister_pending_request(&id2);
    assert_eq!(pending_count(), initial_count);
}

#[tokio::test]
async fn test_registry_concurrent_requests() {
    let initial_count = pending_count();

    let mut handles = Vec::new();
    let mut ids = Vec::new();

    // Register multiple requests concurrently
    for i in 0..10 {
        let (tx, rx) = oneshot::channel::<Value>();
        let id = format!("concurrent-{}-{}", i, uuid::Uuid::new_v4());
        ids.push(id.clone());
        register_pending_request(id.clone(), tx);

        handles.push(tokio::spawn(async move { rx.await }));
    }

    assert_eq!(pending_count(), initial_count + 10);

    // Handle all responses
    for id in &ids {
        handle_pending_response(id, serde_json::json!({"id": id})).unwrap();
    }

    assert_eq!(pending_count(), initial_count);

    // All receivers should have received their responses
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }
}

// =============================================================================
// BidirWithFallback Tests
// =============================================================================

#[tokio::test]
async fn test_fallback_when_not_supported() {
    let (tx, _rx) = mpsc::channel(32);
    let channel = Arc::new(BidirChannel::new_direct(
        tx, false, // not supported
        vec![],
        "hash".into(),
    ));

    let fallback = BidirWithFallback::auto_confirm(channel);

    // Should use fallback (auto-confirm true) when bidirectional not supported
    let resp = fallback
        .request(StandardRequest::Confirm {
            message: "Test?".into(),
            default: None,
        })
        .await;

    assert_eq!(resp, StandardResponse::Confirmed { value: true });
}

#[tokio::test]
async fn test_fallback_uses_default_value() {
    let (tx, _rx) = mpsc::channel(32);
    let channel = Arc::new(BidirChannel::new_direct(tx, false, vec![], "hash".into()));

    let fallback = BidirWithFallback::auto_confirm(channel);

    // With explicit default=false, should use that
    let resp = fallback
        .request(StandardRequest::Confirm {
            message: "Test?".into(),
            default: Some(false),
        })
        .await;

    assert_eq!(resp, StandardResponse::Confirmed { value: false });
}

#[tokio::test]
async fn test_fallback_prompt_uses_default() {
    let (tx, _rx) = mpsc::channel(32);
    let channel = Arc::new(BidirChannel::new_direct(tx, false, vec![], "hash".into()));

    let fallback = BidirWithFallback::auto_confirm(channel);

    let resp = fallback
        .request(StandardRequest::Prompt {
            message: "Name?".into(),
            default: Some(serde_json::Value::String("DefaultName".into())),
            placeholder: None,
        })
        .await;

    assert_eq!(
        resp,
        StandardResponse::Text {
            value: serde_json::Value::String("DefaultName".into()),
        }
    );
}

#[tokio::test]
async fn test_fallback_select_uses_first_option() {
    let (tx, _rx) = mpsc::channel(32);
    let channel = Arc::new(BidirChannel::new_direct(tx, false, vec![], "hash".into()));

    let fallback = BidirWithFallback::auto_confirm(channel);

    let resp = fallback
        .request(StandardRequest::Select {
            message: "Choose:".into(),
            options: vec![
                SelectOption::new("first", "First"),
                SelectOption::new("second", "Second"),
            ],
            multi_select: false,
        })
        .await;

    assert_eq!(
        resp,
        StandardResponse::Selected {
            values: vec![serde_json::Value::String("first".into())],
        }
    );
}

#[tokio::test]
async fn test_fallback_custom_function() {
    let (tx, _rx) = mpsc::channel(32);
    let channel = Arc::new(BidirChannel::new_direct(tx, false, vec![], "hash".into()));

    let fallback = BidirWithFallback::new(channel, |req: &StandardRequest| {
        match req {
            StandardRequest::Confirm { .. } => StandardResponse::Confirmed { value: false }, // Always decline
            StandardRequest::Prompt { .. } => StandardResponse::Text {
                value: serde_json::Value::String("custom".into()),
            },
            StandardRequest::Select { .. } => StandardResponse::Selected {
                values: vec![serde_json::Value::String("custom".into())],
            },
            StandardRequest::Custom { data } => StandardResponse::Custom { data: data.clone() },
        }
    });

    let resp = fallback
        .request(StandardRequest::Confirm {
            message: "Test?".into(),
            default: None,
        })
        .await;

    assert_eq!(resp, StandardResponse::Confirmed { value: false });
}

// =============================================================================
// Helper Function Tests
// =============================================================================

#[tokio::test]
async fn test_create_test_bidir_channel() {
    let (channel, _rx) = create_test_bidir_channel::<StandardRequest, StandardResponse>();
    assert!(channel.is_bidirectional());
    assert_eq!(channel.provenance(), &["test"]);
    assert_eq!(channel.plexus_hash(), "test-hash");
}

#[tokio::test]
async fn test_create_test_standard_channel() {
    let (channel, _rx) = create_test_standard_channel();
    assert!(channel.is_bidirectional());
}

#[tokio::test]
async fn test_auto_respond_channel() {
    let ctx = auto_respond_channel(|req: &StandardRequest| match req {
        StandardRequest::Confirm { .. } => StandardResponse::Confirmed { value: true },
        StandardRequest::Prompt { message, .. } => StandardResponse::Text {
            value: serde_json::Value::String(message.clone()),
        },
        StandardRequest::Select { options, .. } => StandardResponse::Selected {
            values: vec![options[0].value.clone()],
        },
        StandardRequest::Custom { data } => StandardResponse::Custom { data: data.clone() },
    });

    assert_eq!(ctx.confirm("Test?").await.unwrap(), true);
    assert_eq!(ctx.prompt("Echo this").await.unwrap(), "Echo this");

    let selected = ctx
        .select(
            "Pick:",
            vec![SelectOption::new("a", "A"), SelectOption::new("b", "B")],
        )
        .await
        .unwrap();
    assert_eq!(selected, vec!["a".to_string()]);
}

#[tokio::test]
async fn test_auto_confirm_channel_true() {
    let ctx = auto_confirm_channel(true);

    assert_eq!(ctx.confirm("Test?").await.unwrap(), true);
    assert_eq!(ctx.confirm("Another?").await.unwrap(), true);
}

#[tokio::test]
async fn test_auto_confirm_channel_false() {
    let ctx = auto_confirm_channel(false);

    assert_eq!(ctx.confirm("Test?").await.unwrap(), false);
}

#[tokio::test]
async fn test_auto_confirm_channel_respects_default() {
    let ctx = auto_confirm_channel(true);

    // With explicit default=false, should use the default
    let channel_clone = ctx.clone();
    let handle = tokio::spawn(async move {
        channel_clone
            .request(StandardRequest::Confirm {
                message: "Test?".into(),
                default: Some(false),
            })
            .await
    });

    let result = handle.await.unwrap().unwrap();
    assert_eq!(result, StandardResponse::Confirmed { value: false });
}

// =============================================================================
// TimeoutConfig Tests
// =============================================================================

#[test]
fn test_timeout_config_quick() {
    let config = TimeoutConfig::quick();
    assert_eq!(config.confirm, Duration::from_secs(10));
    assert_eq!(config.prompt, Duration::from_secs(10));
    assert_eq!(config.select, Duration::from_secs(10));
    assert_eq!(config.custom, Duration::from_secs(10));
}

#[test]
fn test_timeout_config_normal() {
    let config = TimeoutConfig::normal();
    assert_eq!(config.confirm, Duration::from_secs(30));
    assert_eq!(config.prompt, Duration::from_secs(30));
    assert_eq!(config.select, Duration::from_secs(30));
    assert_eq!(config.custom, Duration::from_secs(30));
}

#[test]
fn test_timeout_config_patient() {
    let config = TimeoutConfig::patient();
    assert_eq!(config.confirm, Duration::from_secs(60));
    assert_eq!(config.prompt, Duration::from_secs(60));
    assert_eq!(config.select, Duration::from_secs(60));
    assert_eq!(config.custom, Duration::from_secs(60));
}

#[test]
fn test_timeout_config_extended() {
    let config = TimeoutConfig::extended();
    assert_eq!(config.confirm, Duration::from_secs(300));
    assert_eq!(config.prompt, Duration::from_secs(300));
    assert_eq!(config.select, Duration::from_secs(300));
    assert_eq!(config.custom, Duration::from_secs(300));
}

#[test]
fn test_timeout_config_default() {
    let config = TimeoutConfig::default();
    assert_eq!(config.confirm, Duration::from_secs(30)); // Same as normal
}

// =============================================================================
// Error Message Tests
// =============================================================================

#[test]
fn test_bidir_error_messages() {
    assert_eq!(
        bidir_error_message(&BidirError::NotSupported),
        "Bidirectional communication not supported by this transport"
    );

    assert_eq!(
        bidir_error_message(&BidirError::Timeout(5000)),
        "Request timed out waiting for response (after 5000ms)"
    );

    assert_eq!(
        bidir_error_message(&BidirError::Cancelled),
        "Request was cancelled by user"
    );

    assert_eq!(
        bidir_error_message(&BidirError::TypeMismatch {
            expected: "Confirmed".into(),
            got: "Text".into()
        }),
        "Type mismatch: expected Confirmed, got Text"
    );

    assert_eq!(
        bidir_error_message(&BidirError::Serialization("parse error".into())),
        "Serialization error: parse error"
    );

    assert_eq!(
        bidir_error_message(&BidirError::Transport("connection lost".into())),
        "Transport error: connection lost"
    );

    assert_eq!(
        bidir_error_message(&BidirError::UnknownRequest),
        "Unknown request ID (may have already been handled)"
    );

    assert_eq!(
        bidir_error_message(&BidirError::ChannelClosed),
        "Response channel closed before response received"
    );
}

// =============================================================================
// SelectOption Tests
// =============================================================================

#[test]
fn test_select_option_new() {
    let opt = SelectOption::new("value", "Label");
    assert_eq!(opt.value, "value");
    assert_eq!(opt.label, "Label");
    assert!(opt.description.is_none());
}

#[test]
fn test_select_option_with_description() {
    let opt = SelectOption::new("key", "Display").with_description("A helpful description");
    assert_eq!(opt.value, "key");
    assert_eq!(opt.label, "Display");
    assert_eq!(opt.description, Some("A helpful description".into()));
}

#[test]
fn test_select_option_serialization() {
    let opt = SelectOption::new("prod", "Production").with_description("Production environment");

    let json = serde_json::to_value(&opt).unwrap();
    assert_eq!(json["value"], "prod");
    assert_eq!(json["label"], "Production");
    assert_eq!(json["description"], "Production environment");
}

#[test]
fn test_select_option_without_description_serialization() {
    let opt = SelectOption::new("dev", "Development");

    let json = serde_json::to_value(&opt).unwrap();
    assert_eq!(json["value"], "dev");
    assert_eq!(json["label"], "Development");
    // description should be absent due to skip_serializing_if
    assert!(json.get("description").is_none());
}

// =============================================================================
// StandardRequest/StandardResponse Serialization Tests
// =============================================================================

#[test]
fn test_standard_request_confirm_serialization() {
    let req: StandardRequest = StandardRequest::Confirm {
        message: "Continue?".into(),
        default: Some(true),
    };

    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["type"], "confirm");
    assert_eq!(json["message"], "Continue?");
    assert_eq!(json["default"], true);

    // Roundtrip
    let decoded: StandardRequest = serde_json::from_value(json).unwrap();
    assert_eq!(req, decoded);
}

#[test]
fn test_standard_request_prompt_serialization() {
    let req: StandardRequest = StandardRequest::Prompt {
        message: "Enter name:".into(),
        default: Some(serde_json::Value::String("John".into())),
        placeholder: Some("Type here...".into()),
    };

    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["type"], "prompt");
    assert_eq!(json["message"], "Enter name:");
    assert_eq!(json["default"], "John");
    assert_eq!(json["placeholder"], "Type here...");
}

#[test]
fn test_standard_request_select_serialization() {
    let req: StandardRequest = StandardRequest::Select {
        message: "Choose:".into(),
        options: vec![
            SelectOption::new("a", "Option A"),
            SelectOption::new("b", "Option B"),
        ],
        multi_select: true,
    };

    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["type"], "select");
    assert_eq!(json["message"], "Choose:");
    assert_eq!(json["multi_select"], true);
    assert_eq!(json["options"].as_array().unwrap().len(), 2);
}

#[test]
fn test_standard_response_confirmed_serialization() {
    let resp: StandardResponse = StandardResponse::Confirmed { value: true };
    let json = serde_json::to_value(&resp).unwrap();
    // Internally tagged: { "type": "confirmed", "value": true }
    assert_eq!(json["type"], "confirmed");
    assert_eq!(json["value"], true);

    let decoded: StandardResponse = serde_json::from_value(json).unwrap();
    assert_eq!(resp, decoded);
}

#[test]
fn test_standard_response_text_serialization() {
    let resp: StandardResponse = StandardResponse::Text {
        value: serde_json::Value::String("Hello".into()),
    };
    let json = serde_json::to_value(&resp).unwrap();
    // Internally tagged: { "type": "text", "value": "Hello" }
    assert_eq!(json["type"], "text");
    assert_eq!(json["value"], "Hello");
}

#[test]
fn test_standard_response_selected_serialization() {
    let resp: StandardResponse = StandardResponse::Selected {
        values: vec![
            serde_json::Value::String("a".into()),
            serde_json::Value::String("b".into()),
        ],
    };
    let json = serde_json::to_value(&resp).unwrap();
    // Internally tagged: { "type": "selected", "values": ["a", "b"] }
    assert_eq!(json["type"], "selected");
    let values = json["values"].as_array().unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0], "a");
    assert_eq!(values[1], "b");
}

#[test]
fn test_standard_response_cancelled_serialization() {
    let resp: StandardResponse = StandardResponse::Cancelled;
    let json = serde_json::to_value(&resp).unwrap();
    // Internally tagged unit variant: { "type": "cancelled" }
    assert_eq!(json["type"], "cancelled");

    let decoded: StandardResponse = serde_json::from_value(json).unwrap();
    assert_eq!(resp, decoded);
}

// =============================================================================
// Custom Type Tests
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TestRequest {
    Ping { value: i32 },
    Echo { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TestResponse {
    Pong { value: i32 },
    Echoed { message: String },
}

#[tokio::test]
async fn test_custom_types_basic_flow() {
    let (tx, mut rx) = mpsc::channel::<PlexusStreamItem>(32);
    let channel: Arc<BidirChannel<TestRequest, TestResponse>> =
        Arc::new(BidirChannel::new_direct(tx, true, vec![], "hash".into()));

    let channel_clone = channel.clone();
    let handle = tokio::spawn(async move {
        channel_clone.request(TestRequest::Ping { value: 42 }).await
    });

    if let Some(PlexusStreamItem::Request {
        request_id,
        request_data,
        ..
    }) = rx.recv().await
    {
        assert_eq!(request_data["type"], "ping");
        assert_eq!(request_data["value"], 42);

        channel
            .handle_response(
                request_id,
                serde_json::to_value(&TestResponse::Pong { value: 42 }).unwrap(),
            )
            .unwrap();
    }

    let result = handle.await.unwrap().unwrap();
    assert_eq!(result, TestResponse::Pong { value: 42 });
}

#[tokio::test]
async fn test_custom_types_echo() {
    let (tx, mut rx) = mpsc::channel(32);
    let channel: Arc<BidirChannel<TestRequest, TestResponse>> =
        Arc::new(BidirChannel::new_direct(tx, true, vec![], "hash".into()));

    let channel_clone = channel.clone();
    let handle = tokio::spawn(async move {
        channel_clone
            .request(TestRequest::Echo {
                message: "Hello".into(),
            })
            .await
    });

    if let Some(PlexusStreamItem::Request { request_id, .. }) = rx.recv().await {
        channel
            .handle_response(
                request_id,
                serde_json::to_value(&TestResponse::Echoed {
                    message: "Hello".into(),
                })
                .unwrap(),
            )
            .unwrap();
    }

    let result = handle.await.unwrap().unwrap();
    assert_eq!(
        result,
        TestResponse::Echoed {
            message: "Hello".into()
        }
    );
}

// =============================================================================
// PlexusStreamItem Request Tests
// =============================================================================

#[test]
fn test_plexus_stream_item_request_creation() {
    let item = PlexusStreamItem::request(
        "req-123".into(),
        serde_json::json!({"key": "value"}),
        5000,
    );

    match item {
        PlexusStreamItem::Request {
            request_id,
            request_data,
            timeout_ms,
        } => {
            assert_eq!(request_id, "req-123");
            assert_eq!(request_data["key"], "value");
            assert_eq!(timeout_ms, 5000);
        }
        _ => panic!("Expected Request variant"),
    }
}

#[test]
fn test_plexus_stream_item_request_serialization() {
    let item = PlexusStreamItem::request(
        "uuid-here".into(),
        serde_json::json!({"type": "confirm", "message": "Test?"}),
        30000,
    );

    let json = serde_json::to_value(&item).unwrap();
    assert_eq!(json["type"], "request");
    // Request fields are camelCase for JavaScript/TypeScript compatibility
    assert_eq!(json["requestId"], "uuid-here");
    assert_eq!(json["timeoutMs"], 30000);
    assert_eq!(json["requestData"]["type"], "confirm");
    assert_eq!(json["requestData"]["message"], "Test?");
}

#[test]
fn test_plexus_stream_item_request_no_metadata() {
    let item = PlexusStreamItem::request("id".into(), serde_json::json!({}), 1000);

    // Request items don't have metadata
    assert!(item.metadata().is_none());
}

// =============================================================================
// BidirError Tests
// =============================================================================

#[test]
fn test_bidir_error_display() {
    assert_eq!(
        BidirError::NotSupported.to_string(),
        "Bidirectional communication not supported by this transport"
    );

    assert_eq!(
        BidirError::Timeout(30000).to_string(),
        "Request timed out after 30000ms"
    );

    assert_eq!(
        BidirError::Cancelled.to_string(),
        "Request was cancelled by client"
    );

    assert_eq!(
        BidirError::TypeMismatch {
            expected: "A".into(),
            got: "B".into()
        }
        .to_string(),
        "Type mismatch: expected A, got B"
    );

    assert_eq!(
        BidirError::Serialization("error".into()).to_string(),
        "Serialization error: error"
    );

    assert_eq!(
        BidirError::Transport("failed".into()).to_string(),
        "Transport error: failed"
    );

    assert_eq!(
        BidirError::UnknownRequest.to_string(),
        "Unknown request ID"
    );

    assert_eq!(
        BidirError::ChannelClosed.to_string(),
        "Response channel closed"
    );
}

#[test]
fn test_bidir_error_clone() {
    let err = BidirError::Timeout(5000);
    let cloned = err.clone();
    assert!(matches!(cloned, BidirError::Timeout(5000)));
}
