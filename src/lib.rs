pub mod app;
pub mod config;
pub mod config_legacy;
pub mod models;
pub mod ui;
pub mod error;
pub mod logging;
pub mod chat;
pub mod api;
pub mod events;
pub mod session;
pub mod message;
pub mod search;
pub mod export;
pub mod plugin;
pub mod streaming;
pub mod templates;
pub mod performance;
pub mod security;

#[cfg(test)]
mod api_streaming_tests;

#[cfg(test)]
mod cli_tests;

#[cfg(test)]
mod lib_tests;

pub use error::{EnhancedError, ErrorCategory, ErrorSeverity, Result};