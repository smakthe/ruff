use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::pin::Pin;
use std::task::{Context, Poll};
use futures_util::Stream;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use crate::{RuffError, models::{AIModel, TokenUsage}};

#[derive(Debug, Serialize, Deserialize)]
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

/// Represents a chunk of streaming response
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub content: String,
    pub is_complete: bool,
    pub token_usage: Option<TokenUsage>,
}

/// Stream of response chunks
pub struct ResponseStream {
    receiver: mpsc::UnboundedReceiver<Result<StreamChunk, RuffError>>,
    cancellation_token: CancellationToken,
}

impl ResponseStream {
    pub fn new(
        receiver: mpsc::UnboundedReceiver<Result<StreamChunk, RuffError>>,
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
    type Item = Result<StreamChunk, RuffError>;
    
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
            client: Client::new(),
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
    ) -> Result<ResponseStream, RuffError> {
        match model.provider.as_str() {
            "openai" => self.send_openai_streaming_message(model, messages, api_key, max_tokens, temperature).await,
            "anthropic" => self.send_anthropic_streaming_message(model, messages, api_key, max_tokens, temperature).await,
            "groq" => self.send_groq_streaming_message(model, messages, api_key, max_tokens, temperature).await,
            "together" => self.send_together_streaming_message(model, messages, api_key, max_tokens, temperature).await,
            _ => Err(RuffError::UnsupportedModel { 
                model: format!("{} (streaming not supported)", model.provider) 
            }),
        }
    }
    
    pub async fn send_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<(String, TokenUsage), RuffError> {
        match model.provider.as_str() {
            "openai" => self.send_openai_message(model, messages, api_key, max_tokens, temperature).await,
            "anthropic" => self.send_anthropic_message(model, messages, api_key, max_tokens, temperature).await,
            "cohere" => self.send_cohere_message(model, messages, api_key, max_tokens, temperature).await,
            "together" => self.send_together_message(model, messages, api_key, max_tokens, temperature).await,
            "groq" => self.send_groq_message(model, messages, api_key, max_tokens, temperature).await,
            "huggingface" => self.send_huggingface_message(model, messages, api_key, max_tokens, temperature).await,
            _ => Err(RuffError::UnsupportedModel { 
                model: model.provider.clone() 
            }),
        }
    }
    
    async fn send_openai_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<(String, TokenUsage), RuffError> {
        let request = ChatRequest {
            model: model.id.clone(),
            messages,
            max_tokens,
            temperature,
            stream: None,
        };
        
        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;
            
        self.handle_response(response).await
    }
    
    async fn send_anthropic_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<(String, TokenUsage), RuffError> {
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
        
        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await?;
            
        let response_text = response.text().await?;
        let response_data: Value = serde_json::from_str(&response_text)?;
        
        let content = response_data["content"][0]["text"]
            .as_str()
            .unwrap_or("No response")
            .to_string();
            
        let usage = TokenUsage {
            input_tokens: response_data["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: response_data["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: 0,
        };
        
        Ok((content, usage))
    }
    
    async fn send_groq_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<(String, TokenUsage), RuffError> {
        let request = ChatRequest {
            model: model.id.clone(),
            messages,
            max_tokens,
            temperature,
            stream: None,
        };
        
        let response = self.client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;
            
        self.handle_response(response).await
    }
    
    async fn send_together_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<(String, TokenUsage), RuffError> {
        let request = ChatRequest {
            model: model.id.clone(),
            messages,
            max_tokens,
            temperature,
            stream: None,
        };
        
        let response = self.client
            .post("https://api.together.xyz/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;
            
        self.handle_response(response).await
    }
    
    async fn send_cohere_message(
        &self,
        _model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<(String, TokenUsage), RuffError> {
        // Cohere API format - simplified for chat
        let message = messages.last().map(|m| m.content.as_str()).unwrap_or("");
        
        let request = serde_json::json!({
            "message": message,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "chat_history": []
        });
        
        let response = self.client
            .post("https://api.cohere.ai/v1/chat")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;
            
        let response_text = response.text().await?;
        let response_data: Value = serde_json::from_str(&response_text)?;
        
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
    }
    
    async fn send_huggingface_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<(String, TokenUsage), RuffError> {
        let prompt = messages.iter()
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
        
        let response = self.client
            .post(&format!("https://api-inference.huggingface.co/models/{}", model.id))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;
            
        let response_data: Vec<Value> = response.json().await?;
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
    }
    
    async fn send_openai_streaming_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<ResponseStream, RuffError> {
        let request = ChatRequest {
            model: model.id.clone(),
            messages,
            max_tokens,
            temperature,
            stream: Some(true),
        };
        
        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;
            
        self.handle_streaming_response(response).await
    }
    
    async fn send_anthropic_streaming_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<ResponseStream, RuffError> {
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
        
        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await?;
            
        self.handle_anthropic_streaming_response(response).await
    }
    
    async fn send_groq_streaming_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<ResponseStream, RuffError> {
        let request = ChatRequest {
            model: model.id.clone(),
            messages,
            max_tokens,
            temperature,
            stream: Some(true),
        };
        
        let response = self.client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;
            
        self.handle_streaming_response(response).await
    }
    
    async fn send_together_streaming_message(
        &self,
        model: &AIModel,
        messages: Vec<ChatMessage>,
        api_key: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<ResponseStream, RuffError> {
        let request = ChatRequest {
            model: model.id.clone(),
            messages,
            max_tokens,
            temperature,
            stream: Some(true),
        };
        
        let response = self.client
            .post("https://api.together.xyz/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;
            
        self.handle_streaming_response(response).await
    }
    
    async fn handle_streaming_response(&self, _response: reqwest::Response) -> Result<ResponseStream, RuffError> {
        // For now, create a mock streaming implementation
        // In a real implementation, this would parse Server-Sent Events from the response
        let (sender, receiver) = mpsc::unbounded_channel();
        let cancellation_token = CancellationToken::new();
        let cancel_token_clone = cancellation_token.clone();
        
        tokio::spawn(async move {
            // Mock streaming response - simulate chunks being sent
            let mock_chunks = vec![
                "Hello",
                " there!",
                " This",
                " is",
                " a",
                " streaming",
                " response.",
            ];
            
            for (i, chunk_text) in mock_chunks.iter().enumerate() {
                if cancel_token_clone.is_cancelled() {
                    break;
                }
                
                let chunk = StreamChunk {
                    content: chunk_text.to_string(),
                    is_complete: i == mock_chunks.len() - 1,
                    token_usage: if i == mock_chunks.len() - 1 {
                        Some(TokenUsage {
                            input_tokens: 10,
                            output_tokens: 20,
                            total_tokens: 30,
                        })
                    } else {
                        None
                    },
                };
                
                if sender.send(Ok(chunk)).is_err() {
                    break;
                }
                
                // Simulate delay between chunks
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        });
        
        Ok(ResponseStream::new(receiver, cancellation_token))
    }
    
    async fn handle_anthropic_streaming_response(&self, _response: reqwest::Response) -> Result<ResponseStream, RuffError> {
        // Mock implementation for Anthropic streaming
        let (sender, receiver) = mpsc::unbounded_channel();
        let cancellation_token = CancellationToken::new();
        let cancel_token_clone = cancellation_token.clone();
        
        tokio::spawn(async move {
            let mock_chunks = vec![
                "I'm",
                " Claude,",
                " and",
                " this",
                " is",
                " a",
                " streaming",
                " response!",
            ];
            
            for (i, chunk_text) in mock_chunks.iter().enumerate() {
                if cancel_token_clone.is_cancelled() {
                    break;
                }
                
                let chunk = StreamChunk {
                    content: chunk_text.to_string(),
                    is_complete: i == mock_chunks.len() - 1,
                    token_usage: if i == mock_chunks.len() - 1 {
                        Some(TokenUsage {
                            input_tokens: 8,
                            output_tokens: 15,
                            total_tokens: 23,
                        })
                    } else {
                        None
                    },
                };
                
                if sender.send(Ok(chunk)).is_err() {
                    break;
                }
                
                tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;
            }
        });
        
        Ok(ResponseStream::new(receiver, cancellation_token))
    }

    async fn handle_response(&self, response: reqwest::Response) -> Result<(String, TokenUsage), RuffError> {
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(RuffError::Api { 
                message: format!("HTTP {}: {}", status, error_text)
            });
        }
        
        let chat_response: ChatResponse = response.json().await?;
        
        let content = chat_response.choices
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