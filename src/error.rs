use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuffError {
    #[error("Configuration error: {0}")]
    Config(#[from] confy::ConfyError),
    
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("API error: {message}")]
    Api { message: String },
    
    #[error("Invalid API key for model: {model}")]
    InvalidApiKey { model: String },
    
    #[error("Model not supported: {model}")]
    UnsupportedModel { model: String },
    
    #[error("Rate limit exceeded for model: {model}")]
    RateLimit { model: String },
    
    #[error("Token limit exceeded. Input: {input_tokens}, Output: {output_tokens}, Max: {max_tokens}")]
    TokenLimit {
        input_tokens: u32,
        output_tokens: u32,
        max_tokens: u32,
    },
    
    #[error("Application error: {0}")]
    App(String),
}

impl From<anyhow::Error> for RuffError {
    fn from(error: anyhow::Error) -> Self {
        RuffError::App(error.to_string())
    }
}