//! Worker service and versioned private local protocol. No library or Core dependency.
pub mod browser;
pub mod client;
pub mod daemon;
mod endpoint;
mod migration;
pub mod protocol;
pub mod service;
pub use client::Client;
pub use marquee_media::error::{AppError, Result};
pub use service::{Job, Output, Receipt, State};
