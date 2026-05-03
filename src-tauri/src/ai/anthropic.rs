// Replaces zeroclaw provider routing per FIX-05. ~100 lines.
// OAuth utilities still use zeroclaw — full removal in Phase 7.
//
// Anthropic Messages API direct reqwest implementation.
// Setup-token (sk-ant-oat01-*): Bearer + anthropic-beta header.
// API key: x-api-key header, no anthropic-beta.
// System prompt is a TOP-LEVEL field — not a message role.

use crate::ai::{AIServiceResponse, ServiceMessage};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};

/// Build request headers and body for the Anthropic Messages API.
///
/// Returns (url, headers, body_json).
/// This is a pure function (no I/O) so it can be unit-tested without network access.
pub fn build_anthropic_request(
    token: &str,
    is_setup_token: bool,
    model: &str,
    max_tokens: u32,
    system: &str,
    messages: &[ServiceMessage],
) -> (String, HeaderMap, Value) {
    let url = "https://api.anthropic.com/v1/messages".to_string();

    let mut headers = HeaderMap::new();
    headers.insert(
        "anthropic-version",
        HeaderValue::from_static("2023-06-01"),
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    if is_setup_token {
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
        );
        headers.insert(
            "anthropic-beta",
            HeaderValue::from_static("oauth-2025-04-20"),
        );
    } else {
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(token).unwrap(),
        );
    }

    let messages_json: Vec<Value> = messages
        .iter()
        .map(|m| json!({"role": m.role, "content": m.content}))
        .collect();

    let body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "system": system,
        "messages": messages_json,
    });

    (url, headers, body)
}

/// Map Anthropic HTTP status codes to user-friendly error messages.
fn map_anthropic_error(status: u16, body: &str) -> String {
    match status {
        401 => "Invalid Anthropic bearer token (check setup-token).".to_string(),
        403 => {
            if body.contains("OAuth authentication is currently not allowed") {
                "Your Anthropic account does not support OAuth tokens. Use an API key instead.".to_string()
            } else {
                "Token does not have the required permissions.".to_string()
            }
        }
        429 => "Anthropic rate limit — try again shortly.".to_string(),
        500..=599 => "Anthropic provider unavailable. Try again later.".to_string(),
        _ => {
            // Try to extract error.message from response body
            if let Ok(json) = serde_json::from_str::<Value>(body) {
                if let Some(msg) = json["error"]["message"].as_str() {
                    return msg.chars().take(200).collect();
                }
            }
            format!("Anthropic API error ({}): {}", status, &body[..body.len().min(200)])
        }
    }
}

/// Send a chat request to the Anthropic Messages API.
pub async fn anthropic_chat(
    token: &str,
    is_setup_token: bool,
    model: &str,
    max_tokens: u32,
    system: &str,
    messages: &[ServiceMessage],
) -> Result<AIServiceResponse, String> {
    let (url, headers, body) = build_anthropic_request(token, is_setup_token, model, max_tokens, system, messages);

    let client = reqwest::Client::new();
    let mut req = client.post(&url);
    for (k, v) in &headers {
        req = req.header(k, v);
    }

    let res = req
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() || e.is_connect() {
                "Could not reach Anthropic — check your connection.".to_string()
            } else {
                format!("Network error: {}", e)
            }
        })?;

    let status = res.status().as_u16();
    let text = res.text().await.map_err(|e| format!("Read error: {}", e))?;

    if status != 200 {
        return Err(map_anthropic_error(status, &text));
    }

    let json: Value = serde_json::from_str(&text).map_err(|e| format!("Parse error: {}", e))?;

    let content = json["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let input_tokens = json["usage"]["input_tokens"].as_u64().unwrap_or(0);
    let output_tokens = json["usage"]["output_tokens"].as_u64().unwrap_or(0);
    let tokens_used = input_tokens + output_tokens;

    Ok(AIServiceResponse {
        content,
        model: json["model"].as_str().unwrap_or(model).to_string(),
        input_tokens: Some(input_tokens),
        output_tokens: Some(tokens_used - input_tokens),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ServiceMessage;

    fn sample_messages() -> Vec<ServiceMessage> {
        vec![ServiceMessage {
            role: "user".to_string(),
            content: "hi".to_string(),
        }]
    }

    // Test 1: setup-token path — Bearer + anthropic-beta headers
    #[test]
    fn test_build_request_setup_token_headers() {
        let msgs = sample_messages();
        let (url, headers, body) = build_anthropic_request(
            "setup-token-abc",
            true,
            "claude-haiku-4-5",
            100,
            "system prompt",
            &msgs,
        );

        assert_eq!(url, "https://api.anthropic.com/v1/messages");

        // Authorization: Bearer
        let auth = headers.get("authorization").unwrap().to_str().unwrap();
        assert_eq!(auth, "Bearer setup-token-abc");

        // anthropic-beta header present
        let beta = headers.get("anthropic-beta").unwrap().to_str().unwrap();
        assert_eq!(beta, "oauth-2025-04-20");

        // anthropic-version header
        let version = headers.get("anthropic-version").unwrap().to_str().unwrap();
        assert_eq!(version, "2023-06-01");

        // Body: system at top level
        assert_eq!(body["system"], "system prompt");

        // Body: messages array
        let messages_arr = body["messages"].as_array().unwrap();
        assert_eq!(messages_arr.len(), 1);
        assert_eq!(messages_arr[0]["role"], "user");
        assert_eq!(messages_arr[0]["content"], "hi");
    }

    // Test 2: API-key path — x-api-key header, no anthropic-beta
    #[test]
    fn test_build_request_api_key_headers() {
        let msgs = sample_messages();
        let (_, headers, _) = build_anthropic_request(
            "sk-ant-api03-test",
            false,
            "claude-haiku-4-5",
            100,
            "sys",
            &msgs,
        );

        // x-api-key present
        let api_key = headers.get("x-api-key").unwrap().to_str().unwrap();
        assert_eq!(api_key, "sk-ant-api03-test");

        // No authorization header
        assert!(headers.get("authorization").is_none(), "API key path must not have Authorization header");

        // No anthropic-beta header
        assert!(headers.get("anthropic-beta").is_none(), "API key path must not have anthropic-beta header");
    }

    // Test 3: Error mapping
    #[test]
    fn test_map_anthropic_error_401() {
        let msg = map_anthropic_error(401, "");
        assert!(msg.contains("Invalid Anthropic bearer token"), "Got: {}", msg);
    }

    #[test]
    fn test_map_anthropic_error_429() {
        let msg = map_anthropic_error(429, "");
        assert!(msg.contains("rate limit"), "Got: {}", msg);
    }

    #[test]
    fn test_map_anthropic_error_503() {
        let msg = map_anthropic_error(503, "");
        assert!(msg.contains("unavailable"), "Got: {}", msg);
    }

    #[test]
    fn test_map_anthropic_error_uses_body_message() {
        let body = r#"{"error":{"type":"invalid_request_error","message":"model not found"}}"#;
        let msg = map_anthropic_error(400, body);
        assert!(msg.contains("model not found"), "Got: {}", msg);
    }
}
