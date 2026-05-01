pub mod models;
pub mod network;
pub mod parameters;
pub mod rate_limiter;
pub mod service;
pub mod validation;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod network_tests;

#[cfg(test)]
mod rate_limiter_tests;

#[cfg(test)]
mod parameters_tests;

pub use models::{
    BackupConfig, GlobalConfig, ModelConfig, PluginConfig, ProxyConfig, RateLimit, RetryConfig,
    UIConfig,
};
pub use network::NetworkConfig;
pub use parameters::{ParameterManager, ParameterPreset, ParameterRanges, UseCase};
pub use rate_limiter::{RateLimitStatus, RateLimiter, RequestSlot, RetryHandler};
pub use service::ConfigurationService;
pub use validation::*;

// Re-export the original config for backward compatibility
pub use crate::config_legacy::{Config, ThemeConfig as LegacyThemeConfig};

// Use the new ThemeConfig by default
pub use models::ThemeConfig;
