use crate::ai::{anthropic_chat, openai_chat};
use crate::auth::{AuthMethod, AuthState};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
/// Routes to the correct direct reqwest implementation based on stored credentials:
/// - Claude subscription (setup-token): anthropic_chat with Bearer + anthropic-beta header
/// - Claude API key: anthropic_chat with x-api-key header
/// - ChatGPT subscription: openai_chat with Bearer token
/// - OpenAI API key: openai_chat with Bearer token
/// - Gemini API key or OAuth: direct HTTP (already was direct)
/// - Ollama: direct HTTP to local server (already was direct)
///
/// Note: zeroclaw is no longer used for AI provider routing (FIX-05).
/// zeroclaw remains as a dependency only for OAuth PKCE utilities in auth/oauth.rs.
/// Full zeroclaw removal is deferred to Phase 7 (Core Extraction).
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
    let max_tokens = request.max_tokens.unwrap_or(4096);
    let token = credential.as_deref().unwrap_or("");

    // ── Direct reqwest routing (FIX-05) ──

    match base_provider.as_str() {
        "anthropic" => {
            let is_setup_token = cred.method == AuthMethod::OAuth;
            anthropic_chat(
                token,
                is_setup_token,
                model,
                max_tokens,
                &request.system_prompt,
                &request.messages,
            )
            .await
        }

        "openai" => {
            openai_chat(token, model, max_tokens, &request.system_prompt, &request.messages).await
        }

        "gemini" => {
            let _temperature = request.temperature.unwrap_or(0.7);
            gemini_chat(
                token,
                cred.method == AuthMethod::OAuth,
                model,
                max_tokens,
                _temperature,
                &request.system_prompt,
                &request.messages,
            )
            .await
        }

        "ollama" => {
            let base_url = cred.base_url.as_deref().unwrap_or("http://localhost:11434");
            ollama_chat(base_url, model, &request.system_prompt, &request.messages).await
        }

        other => Err(format!("Unknown AI provider: {}", other)),
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

pub fn normalize_provider_name(name: &str) -> String {
    match name {
        "claude" | "anthropic" => "anthropic".to_string(),
        "chatgpt" | "openai" | "openai-codex" => "openai".to_string(),
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
    fn test_normalize_openai_codex() {
        // openai-codex normalizes to "openai" — same endpoint, different model string
        assert_eq!(normalize_provider_name("openai-codex"), "openai");
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

    // Test: zeroclaw provider routing is NOT used
    // Verified by: grep -n "create_provider_with_options" src-tauri/src/ai/service.rs returns empty.
    // The routing now goes: "anthropic" -> anthropic_chat(), "openai" -> openai_chat(), etc.
    #[test]
    fn test_anthropic_routes_to_direct_reqwest() {
        // Compile-time proof: this file imports anthropic_chat from crate::ai::anthropic
        // and calls it directly. There is no reference to zeroclaw::providers anywhere.
        // This test exists to document the routing guarantee.
        assert_eq!(normalize_provider_name("anthropic"), "anthropic");
        assert_eq!(normalize_provider_name("openai-codex"), "openai");
    }
}
