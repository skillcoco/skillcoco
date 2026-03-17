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
///
/// Credential resolution strategy (subscription-first):
/// 1. If stored credential has an explicit API key or OAuth token, pass it to zeroclaw
/// 2. Otherwise pass None — zeroclaw resolves from env vars, OAuth tokens, CLI creds:
///    - Anthropic: ANTHROPIC_OAUTH_TOKEN (setup-token) → ANTHROPIC_API_KEY
///    - OpenAI:    OPENAI_API_KEY
///    - Gemini:    GEMINI_API_KEY → GOOGLE_API_KEY → Gemini CLI OAuth → GCloud ADC
///    - Ollama:    no key needed (local)
pub async fn ai_request(
    auth: &AuthState,
    request: AIServiceRequest,
) -> Result<AIServiceResponse, String> {
    let cred = auth
        .get_active_credential()?
        .ok_or("No AI provider configured. Go to Settings to add one.")?;

    let provider_name = normalize_provider_name(&cred.provider);

    // Only pass explicit credentials from our store if the user provided them via BYOK.
    // For OAuth/setup-token and env-based auth, pass None and let zeroclaw resolve
    // from env vars, stored OAuth tokens, and CLI credentials automatically.
    let stored_credential: Option<String> = match cred.method {
        crate::auth::AuthMethod::ApiKey => cred.api_key.clone(),
        crate::auth::AuthMethod::OAuth => cred.oauth_token.clone(),
        crate::auth::AuthMethod::None => None, // Ollama — no key
    };

    let options = ProviderRuntimeOptions {
        max_tokens_override: request.max_tokens,
        ..Default::default()
    };

    let provider = if let Some(base_url) = &cred.base_url {
        providers::create_provider_with_url(
            &provider_name,
            stored_credential.as_deref(),
            Some(base_url),
        )
    } else {
        providers::create_provider_with_options(
            &provider_name,
            stored_credential.as_deref(),
            &options,
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_claude() {
        assert_eq!(normalize_provider_name("claude"), "anthropic");
        assert_eq!(normalize_provider_name("anthropic"), "anthropic");
    }

    #[test]
    fn test_normalize_openai() {
        assert_eq!(normalize_provider_name("chatgpt"), "openai");
        assert_eq!(normalize_provider_name("openai"), "openai");
    }

    #[test]
    fn test_normalize_gemini() {
        assert_eq!(normalize_provider_name("gemini"), "gemini");
        assert_eq!(normalize_provider_name("google"), "gemini");
    }

    #[test]
    fn test_normalize_ollama() {
        assert_eq!(normalize_provider_name("ollama"), "ollama");
    }

    #[test]
    fn test_normalize_unknown_passes_through() {
        assert_eq!(normalize_provider_name("custom-provider"), "custom-provider");
    }

    #[test]
    fn test_ai_request_fails_without_credential() {
        let dir = tempfile::tempdir().unwrap();
        let auth = AuthState::new(&dir.path().to_path_buf());
        let request = AIServiceRequest {
            system_prompt: "test".to_string(),
            messages: vec![],
            max_tokens: None,
            temperature: None,
            response_format: None,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(ai_request(&auth, request));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No AI provider configured"));
    }

    #[test]
    fn test_service_message_serialization() {
        let msg = ServiceMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Hello\""));
    }

    #[test]
    fn test_ai_service_request_defaults() {
        let req = AIServiceRequest {
            system_prompt: "You are helpful.".to_string(),
            messages: vec![
                ServiceMessage { role: "user".to_string(), content: "Hi".to_string() },
            ],
            max_tokens: None,
            temperature: None,
            response_format: None,
        };
        assert_eq!(req.messages.len(), 1);
        assert!(req.max_tokens.is_none());
        assert!(req.temperature.is_none());
    }
}
