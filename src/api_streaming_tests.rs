use crate::{
    api::{APIClient, ChatMessage, StreamChunk},
    models::{AIModel, TokenUsage},
    streaming::{StreamingService, TypingIndicator},
    events::EventBus,
    RuffError,
};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use futures_util::StreamExt;
use uuid::Uuid;

#[tokio::test]
async fn test_streaming_api_client_creation() {
    let client = APIClient::new();
    // Basic test to ensure client can be created
    assert!(true); // Client creation doesn't fail
}

#[tokio::test]
async fn test_stream_chunk_creation() {
    let chunk = StreamChunk {
        content: "Hello".to_string(),
        is_complete: false,
        token_usage: None,
    };
    
    assert_eq!(chunk.content, "Hello");
    assert!(!chunk.is_complete);
    assert!(chunk.token_usage.is_none());
}

#[tokio::test]
async fn test_stream_chunk_with_usage() {
    let usage = TokenUsage {
        input_tokens: 10,
        output_tokens: 20,
        total_tokens: 30,
    };
    
    let chunk = StreamChunk {
        content: "Complete".to_string(),
        is_complete: true,
        token_usage: Some(usage.clone()),
    };
    
    assert_eq!(chunk.content, "Complete");
    assert!(chunk.is_complete);
    assert!(chunk.token_usage.is_some());
    
    let chunk_usage = chunk.token_usage.unwrap();
    assert_eq!(chunk_usage.input_tokens, 10);
    assert_eq!(chunk_usage.output_tokens, 20);
    assert_eq!(chunk_usage.total_tokens, 30);
}

#[tokio::test]
async fn test_streaming_service_initialization() {
    let event_bus = Arc::new(EventBus::new());
    let api_client = Arc::new(APIClient::new());
    let streaming_service = StreamingService::new(event_bus, api_client);
    
    assert_eq!(streaming_service.get_active_streams().await.len(), 0);
    
    let stats = streaming_service.get_streaming_stats().await;
    assert_eq!(stats.active_streams, 0);
    assert_eq!(stats.total_content_length, 0);
}

#[tokio::test]
async fn test_streaming_service_stream_tracking() {
    let event_bus = Arc::new(EventBus::new());
    let api_client = Arc::new(APIClient::new());
    let streaming_service = StreamingService::new(event_bus, api_client);
    
    let message_id = Uuid::new_v4();
    
    // Initially no streams
    assert!(!streaming_service.is_streaming(message_id).await);
    assert!(streaming_service.get_streaming_content(message_id).await.is_none());
}

#[tokio::test]
async fn test_streaming_service_cleanup() {
    let event_bus = Arc::new(EventBus::new());
    let api_client = Arc::new(APIClient::new());
    let streaming_service = StreamingService::new(event_bus, api_client);
    
    // Test cleanup with no active streams
    streaming_service.cleanup_streams().await;
    assert_eq!(streaming_service.get_active_streams().await.len(), 0);
}

#[tokio::test]
async fn test_typing_indicator_lifecycle() {
    let mut indicator = TypingIndicator::new();
    
    // Initially hidden
    assert!(!indicator.is_visible());
    assert_eq!(indicator.current_frame(), "");
    
    // Show indicator
    indicator.show();
    assert!(indicator.is_visible());
    assert!(!indicator.current_frame().is_empty());
    
    // Hide indicator
    indicator.hide();
    assert!(!indicator.is_visible());
    assert_eq!(indicator.current_frame(), "");
}

#[tokio::test]
async fn test_typing_indicator_animation_frames() {
    let mut indicator = TypingIndicator::new();
    indicator.show();
    
    let mut frames = Vec::new();
    
    // Collect several frames
    for _ in 0..5 {
        frames.push(indicator.current_frame().to_string());
        indicator.update();
        sleep(Duration::from_millis(10)).await;
    }
    
    // All frames should be non-empty
    for frame in &frames {
        assert!(!frame.is_empty());
    }
}

#[tokio::test]
async fn test_unsupported_streaming_model() {
    let client = APIClient::new();
    let model = AIModel {
        id: "test-model".to_string(),
        name: "Test Model".to_string(),
        provider: "unsupported".to_string(),
        max_tokens: 1000,
        input_cost_per_1k: 0.001,
        output_cost_per_1k: 0.002,
        supports_system: true,
    };
    
    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: "Hello".to_string(),
    }];
    
    let result = client.send_streaming_message(
        &model,
        messages,
        "fake-key",
        100,
        0.7,
    ).await;
    
    assert!(result.is_err());
    if let Err(RuffError::UnsupportedModel { model: model_name }) = result {
        assert!(model_name.contains("unsupported"));
        assert!(model_name.contains("streaming not supported"));
    } else {
        panic!("Expected UnsupportedModel error");
    }
}

#[tokio::test]
async fn test_chat_message_serialization() {
    let message = ChatMessage {
        role: "user".to_string(),
        content: "Test message".to_string(),
    };
    
    let json = serde_json::to_string(&message).unwrap();
    assert!(json.contains("user"));
    assert!(json.contains("Test message"));
    
    let deserialized: ChatMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.role, "user");
    assert_eq!(deserialized.content, "Test message");
}

#[tokio::test]
async fn test_streaming_cancellation() {
    let event_bus = Arc::new(EventBus::new());
    let api_client = Arc::new(APIClient::new());
    let streaming_service = StreamingService::new(event_bus, api_client);
    
    let message_id = Uuid::new_v4();
    
    // Test cancelling non-existent stream
    let result = streaming_service.cancel_streaming(message_id).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_streaming_stats_calculation() {
    let event_bus = Arc::new(EventBus::new());
    let api_client = Arc::new(APIClient::new());
    let streaming_service = StreamingService::new(event_bus, api_client);
    
    let stats = streaming_service.get_streaming_stats().await;
    
    assert_eq!(stats.active_streams, 0);
    assert_eq!(stats.total_content_length, 0);
    assert_eq!(stats.average_response_time, Duration::ZERO);
}

// Mock tests for streaming response parsing
#[tokio::test]
async fn test_openai_stream_response_parsing() {
    // Test parsing of OpenAI streaming response format
    let sample_data = r#"{"choices":[{"delta":{"content":"Hello"}}]}"#;
    
    let parsed: Result<crate::api::StreamResponse, _> = serde_json::from_str(sample_data);
    assert!(parsed.is_ok());
    
    let response = parsed.unwrap();
    assert_eq!(response.choices.len(), 1);
    assert_eq!(response.choices[0].delta.content, Some("Hello".to_string()));
}

#[tokio::test]
async fn test_stream_response_with_finish_reason() {
    let sample_data = r#"{"choices":[{"delta":{"content":"Done"},"finish_reason":"stop"}]}"#;
    
    let parsed: Result<crate::api::StreamResponse, _> = serde_json::from_str(sample_data);
    assert!(parsed.is_ok());
    
    let response = parsed.unwrap();
    assert_eq!(response.choices[0].finish_reason, Some("stop".to_string()));
}

#[tokio::test]
async fn test_stream_response_with_usage() {
    let sample_data = r#"{"choices":[{"delta":{"content":"Test"}}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
    
    let parsed: Result<crate::api::StreamResponse, _> = serde_json::from_str(sample_data);
    assert!(parsed.is_ok());
    
    let response = parsed.unwrap();
    assert!(response.usage.is_some());
    
    let usage = response.usage.unwrap();
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 5);
    assert_eq!(usage.total_tokens, 15);
}