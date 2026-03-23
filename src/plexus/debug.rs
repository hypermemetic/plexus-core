//! Debug endpoints for Plexus backend
//!
//! This module contains debug-only endpoints that are only active when the
//! PLEXUS_DEBUG environment variable is set to "true".
//!
//! These endpoints provide introspection and debugging capabilities for development
//! and troubleshooting purposes. They should NOT be enabled in production environments.
//!
//! ## Security Note
//! Debug endpoints may expose internal state and system information. Always ensure
//! PLEXUS_DEBUG is not set in production deployments.

use super::streaming::PlexusStream;
use super::types::{PlexusStreamItem, StreamMetadata};
use super::context::PlexusContext;
use futures::stream;

/// Debug endpoint: Protocol test
///
/// This endpoint generates a controlled stream with all message types to validate
/// the axon protocol implementation. It emits:
/// - StreamData (with valid test data)
/// - StreamProgress (with percentage 0-100)
/// - StreamData (another data item)
/// - StreamDone (automatically appended by wrap_stream)
///
/// Each item includes proper metadata with provenance, plexus_hash, and timestamp.
pub fn protocol_test() -> PlexusStream {
    let provenance = vec!["_debug".to_string()];
    let plexus_hash = PlexusContext::hash();

    // Create metadata for each item
    let metadata1 = StreamMetadata::new(provenance.clone(), plexus_hash.clone());
    let metadata2 = StreamMetadata::new(provenance.clone(), plexus_hash.clone());
    let metadata3 = StreamMetadata::new(provenance.clone(), plexus_hash.clone());
    let done_metadata = StreamMetadata::new(provenance, plexus_hash);

    // Build the stream with all message types
    let items = vec![
        // First data item
        PlexusStreamItem::Data {
            metadata: metadata1,
            content_type: "_debug.protocol_test".to_string(),
            content: serde_json::json!({
                "message": "First test data item",
                "sequence": 1,
                "test_field": "validation_data"
            }),
        },
        // Progress item
        PlexusStreamItem::Progress {
            metadata: metadata2,
            message: "Processing test sequence".to_string(),
            percentage: Some(50.0),
        },
        // Second data item
        PlexusStreamItem::Data {
            metadata: metadata3,
            content_type: "_debug.protocol_test".to_string(),
            content: serde_json::json!({
                "message": "Second test data item",
                "sequence": 2,
                "test_field": "validation_complete"
            }),
        },
        // Done item (stream completion)
        PlexusStreamItem::Done {
            metadata: done_metadata,
        },
    ];

    Box::pin(stream::iter(items))
}

/// Debug endpoint: Stream test scenarios
///
/// This endpoint generates different streaming patterns to test stream handling:
/// - "slow": Send data items with delays (100-200ms between items, 5 items total)
/// - "large": Send data items with large payloads (10KB JSON objects, 3 items)
/// - "many": Send many small items quickly (100 items, minimal delay)
/// - "progress": Send items with progress updates (0%, 25%, 50%, 75%, 100%)
///
/// All scenarios end with StreamDone. Defaults to "slow" if no scenario specified.
pub async fn stream_test(params: serde_json::Value) -> PlexusStream {
    use tokio::time::{sleep, Duration};

    let scenario = params["scenario"]
        .as_str()
        .unwrap_or("slow");

    let provenance = vec!["_debug".to_string()];
    let plexus_hash = PlexusContext::hash();

    match scenario {
        "slow" => {
            // 5 items with 100-200ms delays between them
            Box::pin(stream::unfold(0, move |count| {
                let prov = provenance.clone();
                let hash = plexus_hash.clone();
                async move {
                    if count >= 5 {
                        // Final Done item
                        let done_metadata = StreamMetadata::new(prov, hash);
                        Some((PlexusStreamItem::Done { metadata: done_metadata }, count + 1))
                    } else {
                        // Delay before sending item
                        sleep(Duration::from_millis(100 + (count as u64 * 20))).await;

                        let metadata = StreamMetadata::new(prov, hash);
                        let item = PlexusStreamItem::Data {
                            metadata,
                            content_type: "_debug.stream_test".to_string(),
                            content: serde_json::json!({
                                "scenario": "slow",
                                "item": count + 1,
                                "total": 5,
                                "delay_ms": 100 + (count * 20),
                            }),
                        };
                        Some((item, count + 1))
                    }
                }
            }))
        }

        "large" => {
            // 3 items with large payloads (10KB each)
            Box::pin(stream::unfold(0, move |count| {
                let prov = provenance.clone();
                let hash = plexus_hash.clone();
                async move {
                    if count >= 3 {
                        // Final Done item
                        let done_metadata = StreamMetadata::new(prov, hash);
                        Some((PlexusStreamItem::Done { metadata: done_metadata }, count + 1))
                    } else {
                        let metadata = StreamMetadata::new(prov, hash);

                        // Generate ~10KB of data (approx 10000 chars)
                        let large_data: String = (0..500)
                            .map(|i| format!("data_chunk_{:04}_", i))
                            .collect();

                        let item = PlexusStreamItem::Data {
                            metadata,
                            content_type: "_debug.stream_test".to_string(),
                            content: serde_json::json!({
                                "scenario": "large",
                                "item": count + 1,
                                "total": 3,
                                "payload": large_data,
                                "size_bytes": large_data.len(),
                            }),
                        };
                        Some((item, count + 1))
                    }
                }
            }))
        }

        "many" => {
            // 100 items with minimal delay
            Box::pin(stream::unfold(0, move |count| {
                let prov = provenance.clone();
                let hash = plexus_hash.clone();
                async move {
                    if count >= 100 {
                        // Final Done item
                        let done_metadata = StreamMetadata::new(prov, hash);
                        Some((PlexusStreamItem::Done { metadata: done_metadata }, count + 1))
                    } else {
                        // Minimal delay to allow task yielding
                        sleep(Duration::from_millis(1)).await;

                        let metadata = StreamMetadata::new(prov, hash);
                        let item = PlexusStreamItem::Data {
                            metadata,
                            content_type: "_debug.stream_test".to_string(),
                            content: serde_json::json!({
                                "scenario": "many",
                                "item": count + 1,
                                "total": 100,
                            }),
                        };
                        Some((item, count + 1))
                    }
                }
            }))
        }

        "progress" => {
            // Progress updates: 0%, 25%, 50%, 75%, 100%
            Box::pin(stream::unfold(0, move |count| {
                let prov = provenance.clone();
                let hash = plexus_hash.clone();
                async move {
                    if count > 4 {
                        // Final Done item
                        let done_metadata = StreamMetadata::new(prov, hash);
                        Some((PlexusStreamItem::Done { metadata: done_metadata }, count + 1))
                    } else {
                        // Small delay between progress updates
                        sleep(Duration::from_millis(50)).await;

                        let metadata = StreamMetadata::new(prov, hash);
                        let percentage = count as f32 * 25.0;

                        let item = PlexusStreamItem::Progress {
                            metadata,
                            message: format!("Processing step {} of 5", count + 1),
                            percentage: Some(percentage),
                        };
                        Some((item, count + 1))
                    }
                }
            }))
        }

        _ => {
            // Unknown scenario - return error
            let metadata = StreamMetadata::new(provenance.clone(), plexus_hash.clone());
            let done_metadata = StreamMetadata::new(provenance, plexus_hash);

            let items = vec![
                PlexusStreamItem::Error {
                    metadata,
                    message: format!("Unknown scenario: '{}'. Valid scenarios: slow, large, many, progress", scenario),
                    code: Some("invalid_scenario".to_string()),
                    recoverable: false,
                },
                PlexusStreamItem::Done { metadata: done_metadata },
            ];
            Box::pin(stream::iter(items))
        }
    }
}

/// Debug endpoint: Metadata edge cases test
///
/// This endpoint generates stream items with various metadata edge cases to test
/// metadata handling and validation. All items use valid metadata structures but
/// demonstrate different edge cases:
/// - Normal metadata with single provenance item
/// - Nested provenance chains (multiple levels)
/// - Long provenance chains (5+ items)
/// - Different plexus_hash values (all valid 16-char hex)
/// - Different timestamps
/// - Minimal metadata (smallest valid structure)
///
/// All items should pass validation - these are edge cases, not violations.
pub fn metadata_test() -> PlexusStream {
    // Test Case 1: Single provenance item (simplest case)
    let metadata1 = StreamMetadata {
        provenance: vec!["_debug".to_string()],
        plexus_hash: "0123456789abcdef".to_string(), // 16-char lowercase hex
        timestamp: 1609459200, // Jan 1, 2021
    };

    // Test Case 2: Nested provenance (3 levels)
    let metadata2 = StreamMetadata {
        provenance: vec!["root".to_string(), "middle".to_string(), "leaf".to_string()],
        plexus_hash: "fedcba9876543210".to_string(), // Different valid hash
        timestamp: 1640995200, // Jan 1, 2022
    };

    // Test Case 3: Long provenance chain (6 items)
    let metadata3 = StreamMetadata {
        provenance: vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
            "e".to_string(),
            "f".to_string(),
        ],
        plexus_hash: "123456789abcdef0".to_string(),
        timestamp: 1672531200, // Jan 1, 2023
    };

    // Test Case 4: Different hash value, complex provenance
    let metadata4 = StreamMetadata {
        provenance: vec!["parent".to_string(), "child".to_string(), "grandchild".to_string()],
        plexus_hash: "abcdef0123456789".to_string(),
        timestamp: 1704067200, // Jan 1, 2024
    };

    // Test Case 5: Recent timestamp, single provenance
    let metadata5 = StreamMetadata {
        provenance: vec!["_debug".to_string()],
        plexus_hash: "9876543210fedcba".to_string(),
        timestamp: 1735689600, // Jan 1, 2025
    };

    // Test Case 6: Minimal metadata (Done event)
    let done_metadata = StreamMetadata {
        provenance: vec!["_debug".to_string()],
        plexus_hash: "0000000000000000".to_string(), // All zeros but still valid hex
        timestamp: 1735689600,
    };

    // Build the stream with different metadata patterns
    let items = vec![
        // Test 1: Single provenance
        PlexusStreamItem::Data {
            metadata: metadata1,
            content_type: "_debug.metadata_test".to_string(),
            content: serde_json::json!({
                "test": "single_provenance",
                "description": "Simplest metadata with single provenance item"
            }),
        },
        // Test 2: Nested provenance
        PlexusStreamItem::Data {
            metadata: metadata2,
            content_type: "_debug.metadata_test".to_string(),
            content: serde_json::json!({
                "test": "nested_provenance",
                "description": "Metadata with 3-level nested provenance chain"
            }),
        },
        // Test 3: Long provenance chain
        PlexusStreamItem::Data {
            metadata: metadata3,
            content_type: "_debug.metadata_test".to_string(),
            content: serde_json::json!({
                "test": "long_provenance",
                "description": "Metadata with 6-item provenance chain"
            }),
        },
        // Test 4: Complex nested provenance
        PlexusStreamItem::Data {
            metadata: metadata4,
            content_type: "_debug.metadata_test".to_string(),
            content: serde_json::json!({
                "test": "parent_child_grandchild",
                "description": "Metadata with parent->child->grandchild provenance"
            }),
        },
        // Test 5: Different hash and timestamp
        PlexusStreamItem::Data {
            metadata: metadata5,
            content_type: "_debug.metadata_test".to_string(),
            content: serde_json::json!({
                "test": "different_hash_timestamp",
                "description": "Metadata with different hash and recent timestamp"
            }),
        },
        // Done
        PlexusStreamItem::Done {
            metadata: done_metadata,
        },
    ];

    Box::pin(stream::iter(items))
}

/// Debug endpoint: Error test scenarios
///
/// This endpoint generates controlled error scenarios to test error handling:
/// - "immediate": Send error immediately without any data
/// - "after_data": Send 2-3 data items, then send error, then StreamDone
/// - "recoverable": Send data, error, more data, StreamDone (showing that errors don't end streams)
///
/// All scenarios end with StreamDone. Defaults to "immediate" if not specified.
pub async fn error_test(params: serde_json::Value) -> PlexusStream {
    let error_type = params["error_type"]
        .as_str()
        .unwrap_or("immediate");

    let provenance = vec!["_debug".to_string()];
    let plexus_hash = PlexusContext::hash();

    match error_type {
        "immediate" => {
            // Send error immediately without any data
            let error_metadata = StreamMetadata::new(provenance.clone(), plexus_hash.clone());
            let done_metadata = StreamMetadata::new(provenance, plexus_hash);

            let items = vec![
                PlexusStreamItem::Error {
                    metadata: error_metadata,
                    message: "Immediate test error - error sent before any data".to_string(),
                    code: Some("test_error".to_string()),
                    recoverable: false,
                },
                PlexusStreamItem::Done { metadata: done_metadata },
            ];
            Box::pin(stream::iter(items))
        }

        "after_data" => {
            // Send 2-3 data items, then error, then StreamDone
            let metadata1 = StreamMetadata::new(provenance.clone(), plexus_hash.clone());
            let metadata2 = StreamMetadata::new(provenance.clone(), plexus_hash.clone());
            let metadata3 = StreamMetadata::new(provenance.clone(), plexus_hash.clone());
            let error_metadata = StreamMetadata::new(provenance.clone(), plexus_hash.clone());
            let done_metadata = StreamMetadata::new(provenance, plexus_hash);

            let items = vec![
                PlexusStreamItem::Data {
                    metadata: metadata1,
                    content_type: "_debug.error_test".to_string(),
                    content: serde_json::json!({
                        "message": "First data item",
                        "sequence": 1,
                    }),
                },
                PlexusStreamItem::Data {
                    metadata: metadata2,
                    content_type: "_debug.error_test".to_string(),
                    content: serde_json::json!({
                        "message": "Second data item",
                        "sequence": 2,
                    }),
                },
                PlexusStreamItem::Data {
                    metadata: metadata3,
                    content_type: "_debug.error_test".to_string(),
                    content: serde_json::json!({
                        "message": "Third data item",
                        "sequence": 3,
                    }),
                },
                PlexusStreamItem::Error {
                    metadata: error_metadata,
                    message: "Simulated failure after sending data".to_string(),
                    code: Some("simulated_failure".to_string()),
                    recoverable: false,
                },
                PlexusStreamItem::Done { metadata: done_metadata },
            ];
            Box::pin(stream::iter(items))
        }

        "recoverable" => {
            // Send data, error, more data, StreamDone (showing errors don't end streams)
            let metadata1 = StreamMetadata::new(provenance.clone(), plexus_hash.clone());
            let error_metadata = StreamMetadata::new(provenance.clone(), plexus_hash.clone());
            let metadata2 = StreamMetadata::new(provenance.clone(), plexus_hash.clone());
            let metadata3 = StreamMetadata::new(provenance.clone(), plexus_hash.clone());
            let done_metadata = StreamMetadata::new(provenance, plexus_hash);

            let items = vec![
                PlexusStreamItem::Data {
                    metadata: metadata1,
                    content_type: "_debug.error_test".to_string(),
                    content: serde_json::json!({
                        "message": "Data before error",
                        "sequence": 1,
                    }),
                },
                PlexusStreamItem::Error {
                    metadata: error_metadata,
                    message: "Recoverable error - stream continues after this".to_string(),
                    code: Some("recoverable_error".to_string()),
                    recoverable: true,
                },
                PlexusStreamItem::Data {
                    metadata: metadata2,
                    content_type: "_debug.error_test".to_string(),
                    content: serde_json::json!({
                        "message": "Data after error - stream recovered",
                        "sequence": 2,
                    }),
                },
                PlexusStreamItem::Data {
                    metadata: metadata3,
                    content_type: "_debug.error_test".to_string(),
                    content: serde_json::json!({
                        "message": "Final data item",
                        "sequence": 3,
                    }),
                },
                PlexusStreamItem::Done { metadata: done_metadata },
            ];
            Box::pin(stream::iter(items))
        }

        _ => {
            // Unknown error_type - return error
            let metadata = StreamMetadata::new(provenance.clone(), plexus_hash.clone());
            let done_metadata = StreamMetadata::new(provenance, plexus_hash);

            let items = vec![
                PlexusStreamItem::Error {
                    metadata,
                    message: format!("Unknown error_type: '{}'. Valid types: immediate, after_data, recoverable", error_type),
                    code: Some("invalid_error_type".to_string()),
                    recoverable: false,
                },
                PlexusStreamItem::Done { metadata: done_metadata },
            ];
            Box::pin(stream::iter(items))
        }
    }
}
