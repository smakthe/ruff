pub mod api;
pub mod app;
pub mod chat;
pub mod config;
pub mod config_legacy;
pub mod error;
pub mod events;
pub mod export;
pub mod logging;
pub mod message;
pub mod models;
pub mod performance;
pub mod plugin;
pub mod search;
pub mod security;
pub mod session;
pub mod streaming;
pub mod templates;
pub mod ui;

#[cfg(any())]
mod api_streaming_tests;

#[cfg(any())]
mod cli_tests;

#[cfg(any())]
mod lib_tests;

pub use error::{EnhancedError, ErrorCategory, ErrorSeverity, Result};
