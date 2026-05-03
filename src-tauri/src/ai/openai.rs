// Replaces zeroclaw provider routing per FIX-05. ~100 lines.
// OAuth utilities still use zeroclaw — full removal in Phase 7.
//
// OpenAI Chat Completions API direct reqwest implementation.
// Both API key and OAuth use Bearer auth.
// System prompt is a message with role=system at index 0.

use crate::ai::{AIServiceResponse, ServiceMessage};
use serde_json::{json, Value};

/// Build request headers and body for the OpenAI Chat Completions API.
///
/// Returns (url, headers_vec, body_json) where headers_vec is (name, value) pairs.
/// Pure function — no I/O — for unit testability without network access.
pub fn build_openai_request(
    token: &str,
    model: &str,
    max_tokens: u32,
    system: &str,
    messages: &[ServiceMessage],
) -> (String, Vec<(String, String)>, Value) {
    let url = "https://api.openai.com/v1/chat/completions".to_string();

    let headers = vec![
        ("Authorization".to_string(), format!("Bearer {}", token)),
        ("Content-Type".to_string(), "application/json".to_string()),
    ];

    // OpenAI Chat format: system message at index 0, then user/assistant
    let mut messages_json: Vec<Value> = Vec::with_capacity(messages.len() + 1);
    messages_json.push(json!({"role": "system", "content": system}));
    for m in messages {
        messages_json.push(json!({"role": m.role, "content": m.content}));
    }

    let body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": messages_json,
    });

    (url, headers, body)
}

/// Map OpenAI HTTP status codes to user-friendly error messages.
fn map_openai_error(status: u16, body: &str) -> String {
    match status {
        401 => "Invalid OpenAI API key or bearer token.".to_string(),
        403 => "Token does not have the required permissions.".to_string(),
        429 => "OpenAI rate limit — try again shortly.".to_string(),
        500..=599 => "OpenAI service unavailable. Try again later.".to_string(),
        _ => {
            if let Ok(json) = serde_json::from_str::<Value>(body) {
                if let Some(msg) = json["error"]["message"].as_str() {
                    return msg.chars().take(200).collect();
                }
            }
            format!("OpenAI API error ({}): {}", status, &body[..body.len().min(200)])
        }
    }
}

/// Send a chat request to the OpenAI Chat Completions API.
///
/// Works for both API key auth and OAuth bearer tokens.
/// For `openai-codex` provider (ChatGPT subscription): caller passes the OAuth token as `token`.
pub async fn openai_chat(
    token: &str,
    model: &str,
    max_tokens: u32,
    system: &str,
    messages: &[ServiceMessage],
) -> Result<AIServiceResponse, String> {
    let (url, headers, body) = build_openai_request(token, model, max_tokens, system, messages);

    let client = reqwest::Client::new();
    let mut req = client.post(&url);
    for (k, v) in &headers {
        req = req.header(k.as_str(), v.as_str());
    }

    let res = req
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() || e.is_connect() {
                "Could not reach OpenAI — check your connection.".to_string()
            } else {
                format!("Network error: {}", e)
            }
        })?;

    let status = res.status().as_u16();
    let text = res.text().await.map_err(|e| format!("Read error: {}", e))?;

    if status != 200 {
        return Err(map_openai_error(status, &text));
    }

    let json: Value = serde_json::from_str(&text).map_err(|e| format!("Parse error: {}", e))?;

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let total_tokens = json["usage"]["total_tokens"].as_u64().unwrap_or(0);
    let input_tokens = json["usage"]["prompt_tokens"].as_u64().unwrap_or(0);
    let output_tokens = json["usage"]["completion_tokens"].as_u64().unwrap_or(0);

    // Note: total_tokens = input_tokens + output_tokens from OpenAI
    let _ = total_tokens;

    Ok(AIServiceResponse {
        content,
        model: json["model"].as_str().unwrap_or(model).to_string(),
        input_tokens: Some(input_tokens),
        output_tokens: Some(output_tokens),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ServiceMessage;

    fn sample_messages() -> Vec<ServiceMessage> {
        vec![ServiceMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }]
    }

    // Test 1: Bearer auth header and endpoint
    #[test]
    fn test_build_request_bearer_auth() {
        let msgs = sample_messages();
        let (url, headers, body) = build_openai_request("sk-test", "gpt-4o", 100, "sys", &msgs);

        assert_eq!(url, "https://api.openai.com/v1/chat/completions");

        let auth = headers.iter().find(|(k, _)| k == "Authorization").map(|(_, v)| v.as_str());
        assert_eq!(auth, Some("Bearer sk-test"), "Got: {:?}", headers);
    }

    // Test 2: system prompt is first message with role=system
    #[test]
    fn test_build_request_system_as_first_message() {
        let msgs = sample_messages();
        let (_, _, body) = build_openai_request("sk-test", "gpt-4o", 100, "system content", &msgs);

        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "system content");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "hello");
    }

    // Test 3: model field in body
    #[test]
    fn test_build_request_model_field() {
        let msgs = sample_messages();
        let (_, _, body) = build_openai_request("sk-test", "gpt-4o", 256, "sys", &msgs);

        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["max_tokens"], 256);
    }

    // Test 4: Error mapping
    #[test]
    fn test_map_openai_error_401() {
        let msg = map_openai_error(401, "");
        assert!(msg.contains("Invalid OpenAI"), "Got: {}", msg);
    }

    #[test]
    fn test_map_openai_error_429() {
        let msg = map_openai_error(429, "");
        assert!(msg.contains("rate limit"), "Got: {}", msg);
    }

    #[test]
    fn test_map_openai_error_uses_body_message() {
        let body = r#"{"error":{"message":"model not found","type":"invalid_request_error"}}"#;
        let msg = map_openai_error(400, body);
        assert!(msg.contains("model not found"), "Got: {}", msg);
    }
}
