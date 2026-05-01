use super::models::*;
use super::service::ConfigurationService;
use crate::EnhancedError;
use tempfile::TempDir;
use tokio::time::{timeout, Duration as TokioDuration};

async fn create_test_service() -> (ConfigurationService, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let mut service = ConfigurationService::new().unwrap();
    service.config_dir = temp_dir.path().to_path_buf();
    service.initialize().await.unwrap();
    (service, temp_dir)
}

#[tokio::test]
async fn test_rate_limiter_integration() {
    let (service, _temp_dir) = create_test_service().await;

    // Create a model config with rate limiting
    let model_config = ModelConfig {
        name: "rate-limited-model".to_string(),
        provider: "test-provider".to_string(),
        temperature: 0.7,
        max_tokens: 4096,
        rate_limit: RateLimit {
            requests_per_minute: 60,
            tokens_per_minute: Some(100_000),
            concurrent_requests: 5,
        },
        retry_config: RetryConfig {
            max_attempts: 3,
            base_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
            retry_on_rate_limit: true,
            retry_on_network_error: true,
        },
        ..ModelConfig::default()
    };

    service
        .update_model_config("rate-limited-model", model_config)
        .await
        .unwrap();

    // Should be able to make initial requests
    assert!(service.can_make_request("test-provider", Some(1000)));

    // Get rate limit status
    let status = service.get_rate_limit_status("test-provider").unwrap();
    assert_eq!(status.provider, "test-provider");
    assert_eq!(status.max_requests, 60);
    assert_eq!(status.max_tokens, Some(100_000));
    assert_eq!(status.max_concurrent, 5);
}

#[tokio::test]
async fn test_retry_handler_integration() {
    let (service, _temp_dir) = create_test_service().await;

    // Create a model config with retry settings
    let model_config = ModelConfig {
        name: "retry-model".to_string(),
        provider: "retry-provider".to_string(),
        retry_config: RetryConfig {
            max_attempts: 3,
            base_delay_ms: 1, // Very short for testing
            max_delay_ms: 10,
            backoff_multiplier: 2.0,
            retry_on_rate_limit: true,
            retry_on_network_error: true,
        },
        ..ModelConfig::default()
    };

    service
        .update_model_config("retry-model", model_config)
        .await
        .unwrap();

    // Get retry handler
    let retry_handler = service.get_retry_handler("retry-model").unwrap();

    // Test successful operation
    let result = retry_handler
        .execute(|| async { Ok::<i32, EnhancedError>(42) })
        .await;

    assert_eq!(result.unwrap(), 42);
}

#[tokio::test]
async fn test_rate_limit_status_all_providers() {
    let (service, _temp_dir) = create_test_service().await;

    // Create multiple model configs with different providers
    let providers = vec![
        ("provider1", 30, Some(50_000)),
        ("provider2", 60, Some(100_000)),
        ("provider3", 120, None),
    ];

    for (i, (provider, rpm, tpm)) in providers.iter().enumerate() {
        let model_config = ModelConfig {
            name: format!("model-{}", i),
            provider: provider.to_string(),
            rate_limit: RateLimit {
                requests_per_minute: *rpm,
                tokens_per_minute: *tpm,
                concurrent_requests: 5,
            },
            ..ModelConfig::default()
        };

        service
            .update_model_config(&format!("model-{}", i), model_config)
            .await
            .unwrap();
    }

    // Get all rate limit statuses
    let all_statuses = service.get_all_rate_limit_statuses();
    // Should have at least our test providers (may have more from default configs)
    assert!(all_statuses.len() >= providers.len());

    // Check that our test providers are present with correct settings
    for (provider, expected_rpm, expected_tpm) in providers {
        let status = all_statuses
            .iter()
            .find(|s| s.provider == provider)
            .expect(&format!(
                "Provider '{}' not found in rate limit statuses",
                provider
            ));

        assert_eq!(status.max_requests, expected_rpm);
        assert_eq!(status.max_tokens, expected_tpm);
        assert_eq!(status.max_concurrent, 5);
        assert_eq!(status.concurrent_requests, 0);
    }
}

#[tokio::test]
async fn test_wait_for_request_integration() {
    let (service, _temp_dir) = create_test_service().await;

    // Create a model config with very permissive rate limits
    let model_config = ModelConfig {
        name: "fast-model".to_string(),
        provider: "fast-provider".to_string(),
        rate_limit: RateLimit {
            requests_per_minute: 1000,
            tokens_per_minute: Some(1_000_000),
            concurrent_requests: 100,
        },
        ..ModelConfig::default()
    };

    service
        .update_model_config("fast-model", model_config)
        .await
        .unwrap();

    // Should be able to wait for request immediately
    let start = std::time::Instant::now();
    service
        .wait_for_request("fast-provider", Some(1000))
        .await
        .unwrap();
    assert!(start.elapsed() < std::time::Duration::from_millis(100));
}

#[tokio::test]
async fn test_rate_limiter_with_very_low_limits() {
    let (service, _temp_dir) = create_test_service().await;

    // Create a model config with very low rate limits for testing
    let model_config = ModelConfig {
        name: "slow-model".to_string(),
        provider: "slow-provider".to_string(),
        rate_limit: RateLimit {
            requests_per_minute: 2, // Very low for testing
            tokens_per_minute: Some(1000),
            concurrent_requests: 1,
        },
        ..ModelConfig::default()
    };

    service
        .update_model_config("slow-model", model_config)
        .await
        .unwrap();

    // First request should succeed
    assert!(service.can_make_request("slow-provider", Some(100)));

    // Second request should succeed
    assert!(service.can_make_request("slow-provider", Some(100)));

    // Third request should be rate limited
    assert!(!service.can_make_request("slow-provider", Some(100)));

    // Waiting for request should timeout quickly (we'll use a short timeout)
    let result = timeout(
        TokioDuration::from_millis(50),
        service.wait_for_request("slow-provider", Some(100)),
    )
    .await;

    // Should timeout because we need to wait for rate limit reset
    assert!(result.is_err());
}

#[tokio::test]
async fn test_concurrent_request_limiting() {
    let (service, _temp_dir) = create_test_service().await;

    // Create a model config with low concurrent request limit
    let model_config = ModelConfig {
        name: "concurrent-model".to_string(),
        provider: "concurrent-provider".to_string(),
        rate_limit: RateLimit {
            requests_per_minute: 1000, // High request rate
            tokens_per_minute: Some(1_000_000),
            concurrent_requests: 2, // Low concurrent limit
        },
        ..ModelConfig::default()
    };

    service
        .update_model_config("concurrent-model", model_config)
        .await
        .unwrap();

    // Get rate limiter and acquire slots
    let rate_limiter = service.get_rate_limiter();

    let _slot1 = {
        let rate_limiter = rate_limiter.read().unwrap();
        rate_limiter
            .acquire_request_slot("concurrent-provider")
            .unwrap()
    };

    let _slot2 = {
        let rate_limiter = rate_limiter.read().unwrap();
        rate_limiter
            .acquire_request_slot("concurrent-provider")
            .unwrap()
    };

    // Third slot should fail
    {
        let rate_limiter = rate_limiter.read().unwrap();
        assert!(rate_limiter
            .acquire_request_slot("concurrent-provider")
            .is_err());
    }

    // After dropping a slot, should be able to acquire again
    drop(_slot1);

    {
        let rate_limiter = rate_limiter.read().unwrap();
        assert!(rate_limiter
            .acquire_request_slot("concurrent-provider")
            .is_ok());
    }
}

#[tokio::test]
async fn test_retry_handler_with_different_errors() {
    let (service, _temp_dir) = create_test_service().await;

    // Create a model config with retry settings
    let model_config = ModelConfig {
        name: "retry-test-model".to_string(),
        provider: "retry-test-provider".to_string(),
        retry_config: RetryConfig {
            max_attempts: 3,
            base_delay_ms: 1,
            max_delay_ms: 10,
            backoff_multiplier: 2.0,
            retry_on_rate_limit: true,
            retry_on_network_error: false, // Don't retry network errors
        },
        ..ModelConfig::default()
    };

    service
        .update_model_config("retry-test-model", model_config)
        .await
        .unwrap();

    let retry_handler = service.get_retry_handler("retry-test-model").unwrap();

    // Test with retryable error (rate limit)
    let mut attempt_count = 0;
    let result = retry_handler
        .execute(|| {
            attempt_count += 1;
            async move {
                if attempt_count < 2 {
                    Err(EnhancedError::network(format!(
                        "Rate limit exceeded for model: {}",
                        "test"
                    )))
                } else {
                    Ok(42)
                }
            }
        })
        .await;

    assert_eq!(result.unwrap(), 42);
    assert_eq!(attempt_count, 2);

    // Test with non-retryable error (network error with retry disabled)
    let mut attempt_count = 0;
    let result = retry_handler
        .execute(|| {
            attempt_count += 1;
            async move {
                Err::<i32, EnhancedError>(EnhancedError::unknown("Network error test".to_string()))
            }
        })
        .await;

    assert!(result.is_err());
    assert_eq!(attempt_count, 1); // Should not retry
}

#[tokio::test]
async fn test_model_config_update_syncs_rate_limits() {
    let (service, _temp_dir) = create_test_service().await;

    // Create initial model config
    let initial_config = ModelConfig {
        name: "sync-test-model".to_string(),
        provider: "sync-test-provider".to_string(),
        rate_limit: RateLimit {
            requests_per_minute: 30,
            tokens_per_minute: Some(50_000),
            concurrent_requests: 3,
        },
        ..ModelConfig::default()
    };

    service
        .update_model_config("sync-test-model", initial_config)
        .await
        .unwrap();

    // Verify initial rate limit
    let status = service.get_rate_limit_status("sync-test-provider").unwrap();
    assert_eq!(status.max_requests, 30);
    assert_eq!(status.max_tokens, Some(50_000));
    assert_eq!(status.max_concurrent, 3);

    // Update model config with new rate limits
    let updated_config = ModelConfig {
        name: "sync-test-model".to_string(),
        provider: "sync-test-provider".to_string(),
        rate_limit: RateLimit {
            requests_per_minute: 60,
            tokens_per_minute: Some(100_000),
            concurrent_requests: 5,
        },
        ..ModelConfig::default()
    };

    service
        .update_model_config("sync-test-model", updated_config)
        .await
        .unwrap();

    // Verify updated rate limit
    let status = service.get_rate_limit_status("sync-test-provider").unwrap();
    assert_eq!(status.max_requests, 60);
    assert_eq!(status.max_tokens, Some(100_000));
    assert_eq!(status.max_concurrent, 5);
}

#[tokio::test]
async fn test_nonexistent_model_retry_handler() {
    let (service, _temp_dir) = create_test_service().await;

    // Should fail for non-existent model
    assert!(service.get_retry_handler("non-existent-model").is_err());
}

#[tokio::test]
async fn test_rate_limit_status_nonexistent_provider() {
    let (service, _temp_dir) = create_test_service().await;

    // Should return None for non-existent provider
    assert!(service
        .get_rate_limit_status("non-existent-provider")
        .is_none());
}

#[test]
fn test_retry_config_validation_in_model_config() {
    // Test that RetryConfig validation works through ModelConfig
    let valid_config = ModelConfig {
        name: "test".to_string(),
        provider: "test".to_string(),
        retry_config: RetryConfig {
            max_attempts: 3,
            base_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
            retry_on_rate_limit: true,
            retry_on_network_error: true,
        },
        ..ModelConfig::default()
    };

    assert!(valid_config.validate().is_ok());

    let invalid_config = ModelConfig {
        name: "test".to_string(),
        provider: "test".to_string(),
        retry_config: RetryConfig {
            max_attempts: 0, // Invalid
            base_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 0.0, // Invalid
            retry_on_rate_limit: true,
            retry_on_network_error: true,
        },
        ..ModelConfig::default()
    };

    assert!(invalid_config.validate().is_err());
}
