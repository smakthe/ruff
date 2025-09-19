pub mod service;
pub mod models;
pub mod parameters;
pub mod validation;
pub mod network;
pub mod rate_limiter;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod network_tests;

#[cfg(test)]
mod rate_limiter_tests;

#[cfg(test)]
mod parameters_tests;

pub use service::ConfigurationService;
pub use models::{
    GlobalConfig, UIConfig, BackupConfig, ModelConfig, RateLimit, 
    RetryConfig, ProxyConfig, PluginConfig
};
pub use parameters::{ParameterManager, ParameterPreset, UseCase, ParameterRanges};
pub use validation::*;
pub use network::NetworkConfig;
pub use rate_limiter::{RateLimiter, RetryHandler, RateLimitStatus, RequestSlot};

// Re-export the original config for backward compatibility
pub use crate::config_legacy::{Config, ThemeConfig as LegacyThemeConfig};

// Use the new ThemeConfig by default
pub use models::ThemeConfig;