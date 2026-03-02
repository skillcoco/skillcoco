use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AIRequest {
    pub system_prompt: String,
    pub messages: Vec<AIMessage>,
    pub max_tokens: i32,
    pub temperature: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AIMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AIResponse {
    pub content: String,
    pub model: String,
    pub tokens_used: i32,
}

/// Trait defining the AI provider interface.
/// All AI providers (Claude, OpenAI, Ollama, Custom) implement this.
pub trait AIProvider: Send + Sync {
    fn name(&self) -> &str;
    // Note: async_trait would be used here in real implementation
    // For now, we use blocking reqwest in a tokio::spawn_blocking
}

/// Factory function to create the appropriate AI provider based on config
pub fn create_provider(
    provider_type: &str,
    api_key: &str,
    model: &str,
    base_url: &str,
) -> Box<dyn AIProvider> {
    match provider_type {
        "claude" => Box::new(super::claude::ClaudeProvider::new(api_key, model)),
        "openai" => Box::new(super::openai::OpenAIProvider::new(api_key, model)),
        "ollama" => Box::new(super::ollama::OllamaProvider::new(base_url, model)),
        _ => Box::new(super::claude::ClaudeProvider::new(api_key, model)),
    }
}
