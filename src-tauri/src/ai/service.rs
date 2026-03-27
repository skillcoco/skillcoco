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

/// Central AI request function — direct HTTP calls, no zeroclaw.
///
/// Uses only explicitly stored credentials. Never reads environment variables.
/// Subscription tokens (setup-token, OAuth) are the primary auth method.
pub async fn ai_request(
    auth: &AuthState,
    request: AIServiceRequest,
) -> Result<AIServiceResponse, String> {
    let cred = auth
        .get_active_credential()?
        .ok_or("No AI provider configured. Go to Settings to connect one.")?;

    let provider = normalize_provider_name(&cred.provider);

    let credential: Option<String> = match cred.method {
        AuthMethod::ApiKey => cred.api_key.clone(),
        AuthMethod::OAuth => cred.oauth_token.clone(),
        AuthMethod::None => None,
    };

    if credential.is_none() && provider != "ollama" {
        return Err(format!(
            "No credentials stored for {}. Go to Settings and connect using \
             a subscription token (recommended) or API key.",
            cred.provider
        ));
    }

    let model = cred.model.as_deref().unwrap_or("auto");
    let temperature = request.temperature.unwrap_or(0.7);
    let max_tokens = request.max_tokens.unwrap_or(4096);

    match provider.as_str() {
        "anthropic" => {
            anthropic_chat(
                credential.as_deref().unwrap(),
                cred.method == AuthMethod::OAuth,
                model,
                max_tokens,
                temperature,
                &request.system_prompt,
                &request.messages,
            )
            .await
        }
        "openai" => {
            openai_chat(
                credential.as_deref().unwrap(),
                model,
                max_tokens,
                temperature,
                &request.system_prompt,
                &request.messages,
            )
            .await
        }
        "gemini" => {
            gemini_chat(
                credential.as_deref().unwrap(),
                cred.method == AuthMethod::OAuth,
                model,
                max_tokens,
                temperature,
                &request.system_prompt,
                &request.messages,
            )
            .await
        }
        "ollama" => {
            let base_url = cred
                .base_url
                .as_deref()
                .unwrap_or("http://localhost:11434");
            ollama_chat(base_url, model, &request.system_prompt, &request.messages).await
        }
        _ => Err(format!("Unsupported provider: {}", provider)),
    }
}

// ── Anthropic (Claude) ──

async fn anthropic_chat(
    credential: &str,
    is_oauth: bool,
    model: &str,
    max_tokens: u32,
    temperature: f64,
    system_prompt: &str,
    messages: &[ServiceMessage],
) -> Result<AIServiceResponse, String> {
    let client = reqwest::Client::new();
    let mut req = client
        .post("https://api.anthropic.com/v1/messages")
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json");

    if is_oauth {
        req = req
            .header("Authorization", format!("Bearer {}", credential))
            .header("anthropic-beta", "oauth-2025-04-20");
    } else {
        req = req.header("x-api-key", credential);
    }

    let msgs: Vec<Value> = messages
        .iter()
        .map(|m| json!({"role": m.role, "content": m.content}))
        .collect();

    let body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "temperature": temperature,
        "system": system_prompt,
        "messages": msgs,
    });

    let res = req.json(&body).send().await.map_err(|e| format!("Network error: {}", e))?;
    let status = res.status().as_u16();
    let text = res.text().await.map_err(|e| format!("Failed to read response: {}", e))?;

    if status != 200 {
        return Err(format!("Anthropic API error ({}): {}", status, text));
    }

    let json: Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse Anthropic response: {}", e))?;

    let content = json["content"]
        .as_array()
        .and_then(|arr| arr.iter().find(|b| b["type"] == "text"))
        .and_then(|b| b["text"].as_str())
        .unwrap_or("")
        .to_string();

    Ok(AIServiceResponse {
        content,
        model: json["model"].as_str().unwrap_or(model).to_string(),
        input_tokens: json["usage"]["input_tokens"].as_u64(),
        output_tokens: json["usage"]["output_tokens"].as_u64(),
    })
}

// ── OpenAI (ChatGPT) ──

async fn openai_chat(
    credential: &str,
    model: &str,
    max_tokens: u32,
    temperature: f64,
    system_prompt: &str,
    messages: &[ServiceMessage],
) -> Result<AIServiceResponse, String> {
    let client = reqwest::Client::new();

    let mut msgs = vec![json!({"role": "system", "content": system_prompt})];
    for m in messages {
        msgs.push(json!({"role": m.role, "content": m.content}));
    }

    let body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "temperature": temperature,
        "messages": msgs,
    });

    let res = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", credential))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    let status = res.status().as_u16();
    let text = res.text().await.map_err(|e| format!("Failed to read response: {}", e))?;

    if status != 200 {
        return Err(format!("OpenAI API error ({}): {}", status, text));
    }

    let json: Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse OpenAI response: {}", e))?;

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(AIServiceResponse {
        content,
        model: json["model"].as_str().unwrap_or(model).to_string(),
        input_tokens: json["usage"]["prompt_tokens"].as_u64(),
        output_tokens: json["usage"]["completion_tokens"].as_u64(),
    })
}

// ── Gemini ──

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

    let mut req = client
        .post(&url)
        .header("content-type", "application/json");

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
    let text = res.text().await.map_err(|e| format!("Failed to read response: {}", e))?;

    if status != 200 {
        return Err(format!("Gemini API error ({}): {}", status, text));
    }

    let json: Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse Gemini response: {}", e))?;

    let content = json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(AIServiceResponse {
        content,
        model: model.to_string(),
        input_tokens: json["usageMetadata"]["promptTokenCount"].as_u64(),
        output_tokens: json["usageMetadata"]["candidatesTokenCount"].as_u64(),
    })
}

// ── Ollama (Local) ──

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

    let body = json!({
        "model": model,
        "messages": msgs,
        "stream": false,
    });

    let res = client
        .post(format!("{}/api/chat", base_url))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Cannot reach Ollama at {}: {}", base_url, e))?;

    let status = res.status().as_u16();
    let text = res.text().await.map_err(|e| format!("Failed to read response: {}", e))?;

    if status != 200 {
        return Err(format!("Ollama error ({}): {}", status, text));
    }

    let json: Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

    let content = json["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(AIServiceResponse {
        content,
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
            messages: vec![ServiceMessage {
                role: "user".to_string(),
                content: "Hi".to_string(),
            }],
            max_tokens: None,
            temperature: None,
            response_format: None,
        };
        assert_eq!(req.messages.len(), 1);
        assert!(req.max_tokens.is_none());
        assert!(req.temperature.is_none());
    }
}
