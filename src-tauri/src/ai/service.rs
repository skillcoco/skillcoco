use crate::auth::{AuthMethod, AuthState};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use zeroclaw::providers::{self, ChatMessage, ChatRequest, ChatResponse, ProviderRuntimeOptions};

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

/// Central AI request function.
///
/// Routes to the correct zeroclaw provider based on stored credentials:
/// - Claude subscription (setup-token): zeroclaw "anthropic" provider with Bearer auth
/// - Claude API key: zeroclaw "anthropic" provider with x-api-key
/// - ChatGPT subscription: zeroclaw "openai-codex" provider (OAuth via AuthService)
/// - OpenAI API key: zeroclaw "openai" provider
/// - Gemini API key: direct HTTP (zeroclaw gemini needs env vars we want to avoid)
/// - Ollama: direct HTTP to local server
pub async fn ai_request(
    auth: &AuthState,
    request: AIServiceRequest,
) -> Result<AIServiceResponse, String> {
    let cred = auth
        .get_active_credential()?
        .ok_or("No AI provider configured. Go to Settings to connect one.")?;

    let base_provider = normalize_provider_name(&cred.provider);

    let credential: Option<String> = match cred.method {
        AuthMethod::ApiKey => cred.api_key.clone(),
        AuthMethod::OAuth => cred.oauth_token.clone(),
        AuthMethod::None => None,
    };

    if credential.is_none() && base_provider != "ollama" {
        return Err(format!(
            "No credentials stored for {}. Go to Settings and connect using \
             a subscription token (recommended) or API key.",
            cred.provider
        ));
    }

    let model = cred.model.as_deref().unwrap_or("auto");
    let temperature = request.temperature.unwrap_or(0.7);

    // Gemini and Ollama: direct HTTP (avoids zeroclaw env var resolution)
    if base_provider == "gemini" {
        return gemini_chat(
            credential.as_deref().unwrap(),
            cred.method == AuthMethod::OAuth,
            model,
            request.max_tokens.unwrap_or(4096),
            temperature,
            &request.system_prompt,
            &request.messages,
        )
        .await;
    }
    if base_provider == "ollama" {
        let base_url = cred.base_url.as_deref().unwrap_or("http://localhost:11434");
        return ollama_chat(base_url, model, &request.system_prompt, &request.messages).await;
    }

    // Claude and OpenAI: use zeroclaw providers for subscription support
    let (provider_name, provider_credential) = resolve_zeroclaw_provider(
        &base_provider,
        &cred.method,
        credential.as_deref(),
    );

    let options = ProviderRuntimeOptions {
        max_tokens_override: request.max_tokens,
        ..Default::default()
    };

    let provider = providers::create_provider_with_options(
        &provider_name,
        provider_credential.as_deref(),
        &options,
    )
    .map_err(|e| format!("Failed to create provider '{}': {}", provider_name, e))?;

    // Build messages
    let mut messages = Vec::with_capacity(request.messages.len() + 1);
    messages.push(ChatMessage::system(&request.system_prompt));
    for msg in &request.messages {
        messages.push(ChatMessage {
            role: msg.role.clone(),
            content: msg.content.clone(),
        });
    }

    let response: ChatResponse = provider
        .chat(
            ChatRequest {
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

/// Choose the right zeroclaw provider name and credential based on auth method.
///
/// - Claude + OAuth (setup-token): "anthropic" with the token
/// - Claude + API key: "anthropic" with the key
/// - OpenAI + OAuth (ChatGPT subscription): "openai-codex" with None (uses AuthService)
/// - OpenAI + API key: "openai" with the key
fn resolve_zeroclaw_provider(
    base_provider: &str,
    method: &AuthMethod,
    credential: Option<&str>,
) -> (String, Option<String>) {
    match (base_provider, method) {
        // Claude: always pass credential directly — zeroclaw detects setup-token prefix
        ("anthropic", _) => ("anthropic".to_string(), credential.map(String::from)),

        // ChatGPT subscription: use codex provider, pass None so it uses AuthService
        ("openai", AuthMethod::OAuth) => ("openai-codex".to_string(), None),

        // OpenAI API key: regular openai provider
        ("openai", _) => ("openai".to_string(), credential.map(String::from)),

        // Fallback
        (other, _) => (other.to_string(), credential.map(String::from)),
    }
}

// ── Gemini (direct HTTP — avoids zeroclaw env var resolution) ──

async fn gemini_chat(
    credential: &str,
    is_oauth: bool,
    model: &str,
    max_tokens: u32,
    _temperature: f64,
    system_prompt: &str,
    messages: &[ServiceMessage],
) -> Result<AIServiceResponse, String> {
    let client = reqwest::Client::new();

    let url = if is_oauth {
        format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            model
        )
    } else {
        format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, credential
        )
    };

    let mut req = client.post(&url).header("content-type", "application/json");
    if is_oauth {
        req = req.header("Authorization", format!("Bearer {}", credential));
    }

    let contents: Vec<Value> = messages
        .iter()
        .map(|m| {
            let role = if m.role == "assistant" { "model" } else { "user" };
            json!({"role": role, "parts": [{"text": m.content}]})
        })
        .collect();

    let body = json!({
        "contents": contents,
        "systemInstruction": {"parts": [{"text": system_prompt}]},
        "generationConfig": {"maxOutputTokens": max_tokens},
    });

    let res = req.json(&body).send().await.map_err(|e| format!("Network error: {}", e))?;
    let status = res.status().as_u16();
    let text = res.text().await.map_err(|e| format!("Read error: {}", e))?;

    if status != 200 {
        return Err(format!("Gemini API error ({}): {}", status, text));
    }

    let json: Value = serde_json::from_str(&text).map_err(|e| format!("Parse error: {}", e))?;

    Ok(AIServiceResponse {
        content: json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        model: model.to_string(),
        input_tokens: json["usageMetadata"]["promptTokenCount"].as_u64(),
        output_tokens: json["usageMetadata"]["candidatesTokenCount"].as_u64(),
    })
}

// ── Ollama (local, no auth) ──

async fn ollama_chat(
    base_url: &str,
    model: &str,
    system_prompt: &str,
    messages: &[ServiceMessage],
) -> Result<AIServiceResponse, String> {
    let client = reqwest::Client::new();

    let mut msgs = vec![json!({"role": "system", "content": system_prompt})];
    for m in messages {
        msgs.push(json!({"role": m.role, "content": m.content}));
    }

    let body = json!({"model": model, "messages": msgs, "stream": false});

    let res = client
        .post(format!("{}/api/chat", base_url))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Cannot reach Ollama at {}: {}", base_url, e))?;

    let status = res.status().as_u16();
    let text = res.text().await.map_err(|e| format!("Read error: {}", e))?;

    if status != 200 {
        return Err(format!("Ollama error ({}): {}", status, text));
    }

    let json: Value = serde_json::from_str(&text).map_err(|e| format!("Parse error: {}", e))?;

    Ok(AIServiceResponse {
        content: json["message"]["content"].as_str().unwrap_or("").to_string(),
        model: json["model"].as_str().unwrap_or(model).to_string(),
        input_tokens: json["prompt_eval_count"].as_u64(),
        output_tokens: json["eval_count"].as_u64(),
    })
}

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
    fn test_resolve_claude_setup_token() {
        let (name, cred) =
            resolve_zeroclaw_provider("anthropic", &AuthMethod::OAuth, Some("sk-ant-oat01-test"));
        assert_eq!(name, "anthropic");
        assert_eq!(cred.as_deref(), Some("sk-ant-oat01-test"));
    }

    #[test]
    fn test_resolve_claude_api_key() {
        let (name, cred) =
            resolve_zeroclaw_provider("anthropic", &AuthMethod::ApiKey, Some("sk-ant-api03-test"));
        assert_eq!(name, "anthropic");
        assert_eq!(cred.as_deref(), Some("sk-ant-api03-test"));
    }

    #[test]
    fn test_resolve_chatgpt_subscription() {
        let (name, cred) =
            resolve_zeroclaw_provider("openai", &AuthMethod::OAuth, Some("jwt-token"));
        assert_eq!(name, "openai-codex");
        assert!(cred.is_none(), "codex uses AuthService, not direct credential");
    }

    #[test]
    fn test_resolve_openai_api_key() {
        let (name, cred) =
            resolve_zeroclaw_provider("openai", &AuthMethod::ApiKey, Some("sk-proj-test"));
        assert_eq!(name, "openai");
        assert_eq!(cred.as_deref(), Some("sk-proj-test"));
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
}
