//! Bidirectional streaming support for Plexus RPC
//!
//! This module enables server-to-client requests during streaming RPC execution,
//! allowing for interactive workflows like confirmations, prompts, and multi-step wizards.
//!
//! # Architecture
//!
//! The bidirectional system is built on generic types:
//!
//! - `BidirChannel<Req, Resp>` - Generic channel for any request/response types
//! - `StandardRequest/StandardResponse` - Common UI patterns (confirm/prompt/select)
//! - `StandardBidirChannel` - Type alias for standard interactive patterns
//!
//! # Examples
//!
//! ## Using StandardBidirChannel
//!
//! ```rust,ignore
//! use plexus_core::bidirectional::{StandardBidirChannel, StandardRequest, StandardResponse};
//!
//! async fn my_method(ctx: &StandardBidirChannel) -> Result<(), BidirError> {
//!     // Simple confirmation
//!     if ctx.confirm("Delete file?").await? {
//!         // proceed with deletion
//!     }
//!
//!     // Text input
//!     let name = ctx.prompt("Enter your name:").await?;
//!
//!     // Select from options
//!     let choices = vec![
//!         SelectOption::new("dev", "Development"),
//!         SelectOption::new("prod", "Production"),
//!     ];
//!     let selected = ctx.select("Choose environment:", choices).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Using Custom Request/Response Types
//!
//! ```rust,ignore
//! use serde::{Deserialize, Serialize};
//! use schemars::JsonSchema;
//!
//! #[derive(Serialize, Deserialize, JsonSchema)]
//! enum ImageRequest {
//!     ConfirmOverwrite { path: String },
//!     ChooseQuality { options: Vec<u8> },
//! }
//!
//! #[derive(Serialize, Deserialize, JsonSchema)]
//! enum ImageResponse {
//!     Confirmed(bool),
//!     Quality(u8),
//! }
//!
//! async fn process_images(
//!     ctx: &BidirChannel<ImageRequest, ImageResponse>,
//!     paths: Vec<String>,
//! ) -> Result<(), BidirError> {
//!     for path in paths {
//!         let quality = ctx.request(ImageRequest::ChooseQuality {
//!             options: vec![80, 90, 100],
//!         }).await?;
//!
//!         if let ImageResponse::Quality(q) = quality {
//!             // process with quality q
//!         }
//!     }
//!     Ok(())
//! }
//! ```

pub mod channel;
pub mod helpers;
pub mod types;

pub use channel::{BidirChannel, BidirWithFallback, StandardBidirChannel};
pub use helpers::{
    TimeoutConfig, auto_confirm_channel, auto_respond_channel, bidir_error_message,
    create_test_bidir_channel, create_test_standard_channel,
};
pub use types::{BidirError, SelectOption, StandardRequest, StandardResponse};
