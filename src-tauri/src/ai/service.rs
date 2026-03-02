use crate::auth::AuthState;
use serde::{Deserialize, Serialize};
use zeroclaw::providers::{self, ChatMessage, ChatResponse, ProviderRuntimeOptions};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIServiceRequest {
    pub system_prompt: String,
    pub messages: Vec<ServiceMessage>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub response_format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIServiceResponse {
    pub content: String,
    pub model: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

/// Central AI request function. All AI calls go through here.
/// Resolves the active provider credential and routes through zeroclaw.
pub async fn ai_request(
    auth: &AuthState,
    request: AIServiceRequest,
) -> Result<AIServiceResponse, String> {
    let cred = auth
        .get_active_credential()?
        .ok_or("No AI provider configured. Go to Settings to add one.")?;

    let provider_name = normalize_provider_name(&cred.provider);
    let api_key = cred.api_key.as_deref().or(cred.oauth_token.as_deref());

    let options = ProviderRuntimeOptions {
        max_tokens_override: request.max_tokens,
        ..Default::default()
    };

    let provider = if let Some(base_url) = &cred.base_url {
        providers::create_provider_with_url(&provider_name, api_key, Some(base_url))
    } else {
        providers::create_provider_with_options(&provider_name, api_key, &options)
    }
    .map_err(|e| format!("Failed to create AI provider: {}", e))?;

    // Build message list with system prompt
    let mut messages = Vec::with_capacity(request.messages.len() + 1);
    messages.push(ChatMessage::system(&request.system_prompt));

    for msg in &request.messages {
        messages.push(ChatMessage {
            role: msg.role.clone(),
            content: msg.content.clone(),
        });
    }

    let model = cred.model.as_deref().unwrap_or("auto");
    let temperature = request.temperature.unwrap_or(0.7);

    // Use chat_with_history for multi-turn, or chat_with_system for single-turn
    let response: ChatResponse = provider
        .chat(
            providers::ChatRequest {
                messages: &messages,
                tools: None,
            },
            model,
            temperature,
        )
        .await
        .map_err(|e| format!("AI request failed: {}", e))?;

    Ok(AIServiceResponse {
        content: response.text_or_empty().to_string(),
        model: model.to_string(),
        input_tokens: response.usage.as_ref().and_then(|u| u.input_tokens),
        output_tokens: response.usage.as_ref().and_then(|u| u.output_tokens),
    })
}

/// Normalize provider names to zeroclaw-compatible identifiers.
fn normalize_provider_name(name: &str) -> String {
    match name {
        "claude" | "anthropic" => "anthropic".to_string(),
        "chatgpt" | "openai" => "openai".to_string(),
        "gemini" | "google" => "gemini".to_string(),
        "ollama" => "ollama".to_string(),
        other => other.to_string(),
    }
}
