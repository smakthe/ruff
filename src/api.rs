type Result<T> = std::result::Result<T, EnhancedError>;
use crate::error::recovery::retry_with_backoff;
use crate::{
    models::{AIModel, TokenUsage},
    EnhancedError,
};
use futures_util::Stream;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    pub temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
pub struct ResponseMessage {
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Deserialize)]
pub struct StreamChoice {
    pub delta: StreamDelta,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StreamDelta {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StreamResponse {
    pub choices: Vec<StreamChoice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

/// Anthropic streaming event types
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum AnthropicStreamEvent {
    MessageStart {
        message: AnthropicMessage,
    },
    ContentBlockStart {
        index: usize,
        content_block: AnthropicContentBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: AnthropicDelta,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageDelta {
        delta: AnthropicMessageDelta,
        usage: Option<AnthropicUsage>,
    },
    MessageStop,
    Ping,
    Error {
        error: Value,
    },
}

#[derive(Debug, Deserialize)]
pub struct AnthropicMessage {
    pub id: String,
    pub role: String,
    pub content: Vec<Value>,
    #[serde(default)]
    pub usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum AnthropicContentBlock {
    Text { text: String },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum AnthropicDelta {
    TextDelta { text: String },
}

#[derive(Debug, Deserialize)]
pub struct AnthropicMessageDelta {
    #[serde(default)]
    pub stop_reason: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Cohere streaming event types
#[derive(Debug, Deserialize)]
#[serde(tag = "event_type")]
#[serde(rename_all = "kebab-case")]
pub enum CohereStreamEvent {
    MessageStart { id: String },
    ContentStart,
    ContentDelta { delta: CohereDelta },
    ContentEnd,
    MessageEnd { finish_reason: Option<String> },
}

#[derive(Debug, Deserialize)]
pub struct CohereDelta {
    #[serde(default)]
    pub message: Option<CohereMessage>,
}

#[derive(Debug, Deserialize)]
pub struct CohereMessage {
    #[serde(default)]
    pub content: Option<CohereContent>,
}

#[derive(Debug, Deserialize)]
pub struct CohereContent {
    #[serde(default)]
    pub text: Option<String>,
}

/// Represents a chunk of streaming response
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub content: String,
    pub is_complete: bool,
    pub token_usage: Option<TokenUsage>,
}

/// Stream of response chunks
pub struct ResponseStream {
    receiver: mpsc::UnboundedReceiver<Result<StreamChunk>>,
    cancellation_token: CancellationToken,
}

impl ResponseStream {
    pub fn new(
        receiver: mpsc::UnboundedReceiver<Result<StreamChunk>>,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            receiver,
            cancellation_token,
        }
    }

    /// Cancel the streaming response
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    /// Check if the stream has been cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }
}

impl Stream for ResponseStream {
    type Item = Result<StreamChunk>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.cancellation_token.is_cancelled() {
            return Poll::Ready(None);
        }

        self.receiver.poll_recv(cx)
    }
}

pub struct APIClient {
    client: Client,
}

impl APIClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .no_proxy()
                .build()
                .expect("failed to create HTTP client"),
        }
    }

    /// Send a streaming message request
    pub async fn send_streaming_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<ResponseStream> {
        match model.provider.as_str() {
            "openai" => {
                self.send_openai_streaming_message(
                    model,
                    messages,
                    api_key,
                    max_tokens,
                    temperature,
                )
                .await
            }
            "anthropic" => {
                self.send_anthropic_streaming_message(
                    model,
                    messages,
                    api_key,
                    max_tokens,
                    temperature,
                )
                .await
            }
            "groq" => {
                self.send_groq_streaming_message(model, messages, api_key, max_tokens, temperature)
                    .await
            }
            "together" => {
                self.send_together_streaming_message(
                    model,
                    messages,
                    api_key,
                    max_tokens,
                    temperature,
                )
                .await
            }
            _ => Err(EnhancedError::config(format!(
                "Streaming not supported for provider: {}",
                model.provider
            ))),
        }
    }

    pub async fn send_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<(String, TokenUsage)> {
        match model.provider.as_str() {
            "openai" => {
                self.send_openai_message(model, messages, api_key, max_tokens, temperature)
                    .await
            }
            "anthropic" => {
                self.send_anthropic_message(model, messages, api_key, max_tokens, temperature)
                    .await
            }
            "cohere" => {
                self.send_cohere_message(model, messages, api_key, max_tokens, temperature)
                    .await
            }
            "together" => {
                self.send_together_message(model, messages, api_key, max_tokens, temperature)
                    .await
            }
            "groq" => {
                self.send_groq_message(model, messages, api_key, max_tokens, temperature)
                    .await
            }
            "huggingface" => {
                self.send_huggingface_message(model, messages, api_key, max_tokens, temperature)
                    .await
            }
            _ => Err(EnhancedError::config(format!(
                "Unsupported provider: {}",
                model.provider
            ))),
        }
    }

    async fn send_openai_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<(String, TokenUsage)> {
        let request = ChatRequest {
            model: model.id.clone(),
            messages,
            max_tokens,
            temperature,
            stream: None,
        };

        let api_key = api_key.to_string();
        let client = self.client.clone();

        retry_with_backoff(
            3,
            Duration::from_secs(1),
            Duration::from_secs(10),
            || async {
                let response = client
                    .post("https://api.openai.com/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| {
                        EnhancedError::network(format!("OpenAI API request failed: {}", e))
                    })?;

                self.handle_response(response).await
            },
        )
        .await
    }

    async fn send_anthropic_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<(String, TokenUsage)> {
        // Anthropic API format
        let mut anthropic_messages = Vec::new();
        for msg in messages {
            if msg.role != "system" {
                anthropic_messages.push(serde_json::json!({
                    "role": msg.role,
                    "content": msg.content
                }));
            }
        }

        let request = serde_json::json!({
            "model": model.id,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "messages": anthropic_messages
        });

        let api_key = api_key.to_string();
        let client = self.client.clone();

        retry_with_backoff(
            3,
            Duration::from_secs(1),
            Duration::from_secs(10),
            || async {
                let response = client
                    .post("https://api.anthropic.com/v1/messages")
                    .header("x-api-key", &api_key)
                    .header("Content-Type", "application/json")
                    .header("anthropic-version", "2023-06-01")
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| {
                        EnhancedError::network(format!("Anthropic API request failed: {}", e))
                    })?;

                let response_text = response.text().await.map_err(|e| {
                    EnhancedError::network(format!("Failed to read response: {}", e))
                })?;
                let response_data: Value = serde_json::from_str(&response_text).map_err(|e| {
                    EnhancedError::parsing(format!("Failed to parse response: {}", e))
                })?;

                let content = response_data["content"][0]["text"]
                    .as_str()
                    .unwrap_or("No response")
                    .to_string();

                let usage = TokenUsage {
                    input_tokens: response_data["usage"]["input_tokens"].as_u64().unwrap_or(0)
                        as u32,
                    output_tokens: response_data["usage"]["output_tokens"]
                        .as_u64()
                        .unwrap_or(0) as u32,
                    total_tokens: 0,
                };

                Ok((content, usage))
            },
        )
        .await
    }

    async fn send_groq_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<(String, TokenUsage)> {
        let request = ChatRequest {
            model: model.id.clone(),
            messages,
            max_tokens,
            temperature,
            stream: None,
        };

        let api_key = api_key.to_string();
        let client = self.client.clone();

        retry_with_backoff(
            3,
            Duration::from_secs(1),
            Duration::from_secs(10),
            || async {
                let response = client
                    .post("https://api.groq.com/openai/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| {
                        EnhancedError::network(format!("Groq API request failed: {}", e))
                    })?;

                self.handle_response(response).await
            },
        )
        .await
    }

    async fn send_together_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<(String, TokenUsage)> {
        let request = ChatRequest {
            model: model.id.clone(),
            messages,
            max_tokens,
            temperature,
            stream: None,
        };

        let api_key = api_key.to_string();
        let client = self.client.clone();

        retry_with_backoff(
            3,
            Duration::from_secs(1),
            Duration::from_secs(10),
            || async {
                let response = client
                    .post("https://api.together.xyz/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| {
                        EnhancedError::network(format!("Together API request failed: {}", e))
                    })?;

                self.handle_response(response).await
            },
        )
        .await
    }

    async fn send_cohere_message(
        &self,
        _model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<(String, TokenUsage)> {
        // Cohere API format - simplified for chat
        let message = messages.last().map(|m| m.content.as_str()).unwrap_or("");

        let request = serde_json::json!({
            "message": message,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "chat_history": []
        });

        let api_key = api_key.to_string();
        let client = self.client.clone();

        retry_with_backoff(
            3,
            Duration::from_secs(1),
            Duration::from_secs(10),
            || async {
                let response = client
                    .post("https://api.cohere.ai/v1/chat")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| {
                        EnhancedError::network(format!("Cohere API request failed: {}", e))
                    })?;

                let response_text = response.text().await.map_err(|e| {
                    EnhancedError::network(format!("Failed to read response: {}", e))
                })?;
                let response_data: Value = serde_json::from_str(&response_text).map_err(|e| {
                    EnhancedError::parsing(format!("Failed to parse response: {}", e))
                })?;

                let content = response_data["text"]
                    .as_str()
                    .unwrap_or("No response")
                    .to_string();

                let usage = TokenUsage {
                    input_tokens: 0, // Cohere doesn't always provide token usage
                    output_tokens: 0,
                    total_tokens: 0,
                };

                Ok((content, usage))
            },
        )
        .await
    }

    async fn send_huggingface_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<(String, TokenUsage)> {
        let prompt = messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let request = serde_json::json!({
            "inputs": prompt,
            "parameters": {
                "max_new_tokens": max_tokens,
                "temperature": temperature,
                "return_full_text": false
            }
        });

        let api_key = api_key.to_string();
        let model_id = model.id.clone();
        let client = self.client.clone();

        retry_with_backoff(
            3,
            Duration::from_secs(1),
            Duration::from_secs(10),
            || async {
                let response = client
                    .post(&format!(
                        "https://api-inference.huggingface.co/models/{}",
                        model_id
                    ))
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| {
                        EnhancedError::network(format!("HuggingFace API request failed: {}", e))
                    })?;

                let response_data: Vec<Value> = response.json().await.map_err(|e| {
                    EnhancedError::parsing(format!("Failed to parse response: {}", e))
                })?;
                let content = response_data[0]["generated_text"]
                    .as_str()
                    .unwrap_or("No response")
                    .to_string();

                let usage = TokenUsage {
                    input_tokens: 0,
                    output_tokens: 0,
                    total_tokens: 0,
                };

                Ok((content, usage))
            },
        )
        .await
    }

    async fn send_openai_streaming_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<ResponseStream> {
        let request = ChatRequest {
            model: model.id.clone(),
            messages,
            max_tokens,
            temperature,
            stream: Some(true),
        };

        let api_key = api_key.to_string();
        let client = self.client.clone();

        retry_with_backoff(
            3,
            Duration::from_secs(1),
            Duration::from_secs(10),
            || async {
                let response = client
                    .post("https://api.openai.com/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| {
                        EnhancedError::network(format!("OpenAI streaming request failed: {}", e))
                    })?;

                self.handle_streaming_response(response).await
            },
        )
        .await
    }

    async fn send_anthropic_streaming_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<ResponseStream> {
        let mut anthropic_messages = Vec::new();
        for msg in messages {
            if msg.role != "system" {
                anthropic_messages.push(serde_json::json!({
                    "role": msg.role,
                    "content": msg.content
                }));
            }
        }

        let request = serde_json::json!({
            "model": model.id,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "messages": anthropic_messages,
            "stream": true
        });

        let api_key = api_key.to_string();
        let client = self.client.clone();

        retry_with_backoff(
            3,
            Duration::from_secs(1),
            Duration::from_secs(10),
            || async {
                let response = client
                    .post("https://api.anthropic.com/v1/messages")
                    .header("x-api-key", &api_key)
                    .header("Content-Type", "application/json")
                    .header("anthropic-version", "2023-06-01")
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| {
                        EnhancedError::network(format!("Anthropic streaming request failed: {}", e))
                    })?;

                self.handle_anthropic_streaming_response(response).await
            },
        )
        .await
    }

    async fn send_groq_streaming_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<ResponseStream> {
        let request = ChatRequest {
            model: model.id.clone(),
            messages,
            max_tokens,
            temperature,
            stream: Some(true),
        };

        let api_key = api_key.to_string();
        let client = self.client.clone();

        retry_with_backoff(
            3,
            Duration::from_secs(1),
            Duration::from_secs(10),
            || async {
                let response = client
                    .post("https://api.groq.com/openai/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| {
                        EnhancedError::network(format!("Groq streaming request failed: {}", e))
                    })?;

                self.handle_streaming_response(response).await
            },
        )
        .await
    }

    async fn send_together_streaming_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<ResponseStream> {
        let request = ChatRequest {
            model: model.id.clone(),
            messages,
            max_tokens,
            temperature,
            stream: Some(true),
        };

        let api_key = api_key.to_string();
        let client = self.client.clone();

        retry_with_backoff(
            3,
            Duration::from_secs(1),
            Duration::from_secs(10),
            || async {
                let response = client
                    .post("https://api.together.xyz/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| {
                        EnhancedError::network(format!("Together streaming request failed: {}", e))
                    })?;

                self.handle_streaming_response(response).await
            },
        )
        .await
    }

    async fn handle_streaming_response(
        &self,
        response: reqwest::Response,
    ) -> Result<ResponseStream> {
        // Real SSE streaming implementation for OpenAI-compatible APIs
        let (sender, receiver) = mpsc::unbounded_channel();
        let cancellation_token = CancellationToken::new();
        let cancel_token_clone = cancellation_token.clone();

        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();
            let mut total_usage: Option<TokenUsage> = None;

            loop {
                if cancel_token_clone.is_cancelled() {
                    break;
                }

                let chunk_result = match stream.next().await {
                    Some(result) => result,
                    None => break, // Stream ended
                };

                let bytes = match chunk_result {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = sender.send(Err(EnhancedError::from(e)));
                        break;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&bytes));

                // Process complete SSE events (lines ending with \n\n)
                while let Some(event_end) = buffer.find("\n\n") {
                    let event_str = buffer[..event_end].to_string();
                    buffer = buffer[event_end + 2..].to_string();

                    // Parse SSE event
                    if event_str.trim().is_empty() {
                        continue;
                    }

                    // Check for [DONE] message
                    if event_str.contains("data: [DONE]") {
                        let chunk = StreamChunk {
                            content: String::new(),
                            is_complete: true,
                            token_usage: total_usage.clone(),
                        };
                        let _ = sender.send(Ok(chunk));
                        break;
                    }

                    // Extract data from SSE format
                    for line in event_str.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            match serde_json::from_str::<StreamResponse>(data) {
                                Ok(response) => {
                                    for choice in response.choices {
                                        if let Some(content) = choice.delta.content {
                                            let is_final = choice.finish_reason.is_some();

                                            // Update usage if available
                                            if let Some(usage) = &response.usage {
                                                total_usage = Some(TokenUsage {
                                                    input_tokens: usage.prompt_tokens,
                                                    output_tokens: usage.completion_tokens,
                                                    total_tokens: usage.total_tokens,
                                                });
                                            }

                                            let chunk = StreamChunk {
                                                content,
                                                is_complete: is_final,
                                                token_usage: if is_final {
                                                    total_usage.clone()
                                                } else {
                                                    None
                                                },
                                            };

                                            if sender.send(Ok(chunk)).is_err() {
                                                return;
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse SSE data: {} - Data: {}", e, data);
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(ResponseStream::new(receiver, cancellation_token))
    }

    async fn handle_anthropic_streaming_response(
        &self,
        response: reqwest::Response,
    ) -> Result<ResponseStream> {
        // Real SSE streaming implementation for Anthropic Claude API
        let (sender, receiver) = mpsc::unbounded_channel();
        let cancellation_token = CancellationToken::new();
        let cancel_token_clone = cancellation_token.clone();

        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();
            let mut total_input_tokens = 0u32;
            let mut total_output_tokens = 0u32;

            loop {
                if cancel_token_clone.is_cancelled() {
                    break;
                }

                let chunk_result = match stream.next().await {
                    Some(result) => result,
                    None => break, // Stream ended
                };

                let bytes = match chunk_result {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = sender.send(Err(EnhancedError::from(e)));
                        break;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&bytes));

                // Process complete SSE events
                while let Some(event_end) = buffer.find("\n\n") {
                    let event_str = buffer[..event_end].to_string();
                    buffer = buffer[event_end + 2..].to_string();

                    if event_str.trim().is_empty() {
                        continue;
                    }

                    // Parse Anthropic SSE event format: "event: {type}\ndata: {json}"
                    let mut event_data = "";

                    for line in event_str.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            event_data = data;
                        }
                    }

                    if !event_data.is_empty() {
                        match serde_json::from_str::<AnthropicStreamEvent>(event_data) {
                            Ok(event) => {
                                match event {
                                    AnthropicStreamEvent::ContentBlockDelta { delta, .. } => {
                                        let AnthropicDelta::TextDelta { text } = delta;
                                        let chunk = StreamChunk {
                                            content: text,
                                            is_complete: false,
                                            token_usage: None,
                                        };
                                        if sender.send(Ok(chunk)).is_err() {
                                            return;
                                        }
                                    }
                                    AnthropicStreamEvent::MessageDelta { usage, .. } => {
                                        if let Some(usage) = usage {
                                            total_input_tokens = usage.input_tokens;
                                            total_output_tokens = usage.output_tokens;
                                        }
                                    }
                                    AnthropicStreamEvent::MessageStop => {
                                        let chunk = StreamChunk {
                                            content: String::new(),
                                            is_complete: true,
                                            token_usage: Some(TokenUsage {
                                                input_tokens: total_input_tokens,
                                                output_tokens: total_output_tokens,
                                                total_tokens: total_input_tokens
                                                    + total_output_tokens,
                                            }),
                                        };
                                        let _ = sender.send(Ok(chunk));
                                        break;
                                    }
                                    AnthropicStreamEvent::Error { error } => {
                                        let _ = sender.send(Err(EnhancedError::api(format!(
                                            "Anthropic streaming error: {:?}",
                                            error
                                        ))));
                                        break;
                                    }
                                    _ => {} // Ignore other events
                                }
                            }
                            Err(e) => {
                                eprintln!(
                                    "Failed to parse Anthropic SSE event: {} - Event: {}",
                                    e, event_data
                                );
                            }
                        }
                    }
                }
            }
        });

        Ok(ResponseStream::new(receiver, cancellation_token))
    }

    async fn handle_response(&self, response: reqwest::Response) -> Result<(String, TokenUsage)> {
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(
                EnhancedError::api(format!("HTTP {}: {}", status, error_text))
                    .with_metadata("status_code".to_string(), status.as_u16().to_string()),
            );
        }

        let chat_response: ChatResponse = response.json().await?;

        let content = chat_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_else(|| "No response".to_string());

        let usage = if let Some(usage) = chat_response.usage {
            TokenUsage {
                input_tokens: usage.prompt_tokens,
                output_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
            }
        } else {
            TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
            }
        };

        Ok((content, usage))
    }
}

impl Default for APIClient {
    fn default() -> Self {
        Self::new()
    }
}
