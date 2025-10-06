//! Error recovery strategies and utilities
//!
//! This module provides error recovery mechanisms including retry logic
//! with exponential backoff, fallback strategies, and error-specific recovery.

use crate::error::{EnhancedError, ErrorCategory};
use std::time::Duration;
use tokio::time::sleep;

/// Recovery strategy for errors
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// Retry with exponential backoff
    RetryWithBackoff {
        max_attempts: usize,
        initial_delay: Duration,
        max_delay: Duration,
    },

    /// Use cached value as fallback
    UseCachedValue,

    /// Use default value
    UseDefault,

    /// Prompt user for action
    PromptUser,

    /// Fail immediately (no recovery)
    FailImmediately,
}

impl RecoveryStrategy {
    /// Get recommended strategy for an error based on its category
    pub fn for_error(error: &EnhancedError) -> Self {
        match error.category {
            ErrorCategory::Network => {
                // Network errors: retry with backoff
                RecoveryStrategy::RetryWithBackoff {
                    max_attempts: 3,
                    initial_delay: Duration::from_secs(1),
                    max_delay: Duration::from_secs(10),
                }
            }

            ErrorCategory::Storage => {
                // Storage errors: try cache, then fail
                RecoveryStrategy::UseCachedValue
            }

            ErrorCategory::Auth => {
                // Auth errors: prompt for new credentials
                RecoveryStrategy::PromptUser
            }

            _ => {
                // Default: fail immediately
                RecoveryStrategy::FailImmediately
            }
        }
    }

    /// Create a network retry strategy with default settings
    pub fn network_retry() -> Self {
        RecoveryStrategy::RetryWithBackoff {
            max_attempts: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(10),
        }
    }

    /// Create a rate limit retry strategy with longer delays
    pub fn rate_limit_retry() -> Self {
        RecoveryStrategy::RetryWithBackoff {
            max_attempts: 5,
            initial_delay: Duration::from_secs(5),
            max_delay: Duration::from_secs(60),
        }
    }
}

/// Execute an async operation with retry logic and exponential backoff
///
/// # Example
/// ```no_run
/// use ruff::error::recovery::retry_with_backoff;
/// use std::time::Duration;
///
/// # async fn example() -> Result<(), ruff::EnhancedError> {
/// let result = retry_with_backoff(
///     3,
///     Duration::from_secs(1),
///     Duration::from_secs(10),
///     || async {
///         // Your operation here
///         Ok(42)
///     }
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn retry_with_backoff<F, Fut, T>(
    max_attempts: usize,
    initial_delay: Duration,
    max_delay: Duration,
    mut operation: F,
) -> Result<T, EnhancedError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, EnhancedError>>,
{
    let mut attempt = 0;
    let mut delay = initial_delay;

    loop {
        attempt += 1;

        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt >= max_attempts {
                    return Err(e.with_details(format!(
                        "Failed after {} attempts",
                        max_attempts
                    )));
                }

                eprintln!(
                    "⚠️  Operation failed (attempt {}/{}): {}. Retrying in {:?}...",
                    attempt, max_attempts, e, delay
                );

                sleep(delay).await;

                // Exponential backoff
                delay = std::cmp::min(delay * 2, max_delay);
            }
        }
    }
}

/// Execute an async operation with a simple retry (fixed delay)
pub async fn retry_simple<F, Fut, T>(
    max_attempts: usize,
    delay: Duration,
    mut operation: F,
) -> Result<T, EnhancedError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, EnhancedError>>,
{
    retry_with_backoff(max_attempts, delay, delay, operation).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_retry_succeeds_on_first_attempt() {
        let mut call_count = 0;

        let result = retry_with_backoff(
            3,
            Duration::from_millis(10),
            Duration::from_millis(100),
            || async {
                call_count += 1;
                Ok::<i32, EnhancedError>(42)
            }
        ).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(call_count, 1);
    }

    #[tokio::test]
    async fn test_retry_succeeds_on_second_attempt() {
        let mut call_count = 0;

        let result = retry_with_backoff(
            3,
            Duration::from_millis(10),
            Duration::from_millis(100),
            || async {
                call_count += 1;
                if call_count < 2 {
                    Err(EnhancedError::network("Temporary failure"))
                } else {
                    Ok(42)
                }
            }
        ).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(call_count, 2);
    }

    #[tokio::test]
    async fn test_retry_fails_after_max_attempts() {
        let mut call_count = 0;

        let result = retry_with_backoff(
            3,
            Duration::from_millis(10),
            Duration::from_millis(100),
            || async {
                call_count += 1;
                Err::<i32, EnhancedError>(EnhancedError::network("Persistent failure"))
            }
        ).await;

        assert!(result.is_err());
        assert_eq!(call_count, 3);
        assert!(result.unwrap_err().to_string().contains("Failed after 3 attempts"));
    }

    #[test]
    fn test_recovery_strategy_for_network_error() {
        let error = EnhancedError::network("Connection failed");
        let strategy = RecoveryStrategy::for_error(&error);

        match strategy {
            RecoveryStrategy::RetryWithBackoff { max_attempts, .. } => {
                assert_eq!(max_attempts, 3);
            }
            _ => panic!("Expected RetryWithBackoff strategy"),
        }
    }

    #[test]
    fn test_recovery_strategy_for_auth_error() {
        let error = EnhancedError::auth("Invalid credentials");
        let strategy = RecoveryStrategy::for_error(&error);

        matches!(strategy, RecoveryStrategy::PromptUser);
    }
}
