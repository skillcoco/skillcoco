use crate::auth::{AuthMethod, AuthState, ProviderCredential};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::State;
use zeroclaw::auth::oauth_common::generate_pkce_state;
use zeroclaw::auth::{gemini_oauth, openai_oauth, AuthService};

// ── OAuthFlowState ──

/// Per-flow entry tracking the completion, authentication, and error state
/// of an in-flight or recently completed OAuth flow.
#[derive(Clone, Default)]
struct FlowEntry {
    completed: bool,
    authenticated: bool,
    error: Option<String>,
}

/// Tracks in-flight OAuth flows keyed by provider id.
#[derive(Clone)]
pub struct OAuthFlowState {
    flows: Arc<Mutex<HashMap<String, FlowEntry>>>,
}

impl OAuthFlowState {
    pub fn new() -> Self {
        Self {
            flows: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Mark the flow as started / clear any prior error (called on fresh login).
    fn start(&self, provider: &str) -> Result<(), String> {
        let mut flows = self.flows.lock().map_err(|e| e.to_string())?;
        flows.insert(
            provider.to_string(),
            FlowEntry {
                completed: false,
                authenticated: false,
                error: None,
            },
        );
        Ok(())
    }

    /// Mark the flow as completed and authenticated.
    fn set_authenticated(&self, provider: &str) {
        if let Ok(mut flows) = self.flows.lock() {
            let entry = flows.entry(provider.to_string()).or_default();
            entry.completed = true;
            entry.authenticated = true;
            entry.error = None;
        }
    }

    /// Mark the flow as completed with an error.
    fn set_error(&self, provider: &str, message: &str) {
        if let Ok(mut flows) = self.flows.lock() {
            let entry = flows.entry(provider.to_string()).or_default();
            entry.completed = true;
            entry.authenticated = false;
            entry.error = Some(message.to_string());
        }
    }

    /// Read the current flow status for a provider.
    fn status(&self, provider: &str) -> Result<(bool, bool, Option<String>), String> {
        let flows = self.flows.lock().map_err(|e| e.to_string())?;
        let entry = flows.get(provider);
        Ok((
            entry.map(|e| e.completed).unwrap_or(false),
            entry.map(|e| e.authenticated).unwrap_or(false),
            entry.and_then(|e| e.error.clone()),
        ))
    }
}

// ── User-facing result types ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthStartResult {
    pub started: bool,
    pub provider: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthStatusResult {
    pub completed: bool,
    pub provider: String,
    pub authenticated: bool,
    /// Optional error message from the OAuth flow. Populated by FIX-01.
    /// Serialized only when Some — frontend checks for `error` field presence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ── Error mapping ──

/// Map OAuth-related errors to user-friendly messages.
///
/// This is a pure function so it can be unit tested without I/O.
pub fn map_oauth_error(err_str: &str) -> String {
    let lower = err_str.to_lowercase();
    if lower.contains("401") || lower.contains("unauthorized") || lower.contains("invalid token") || lower.contains("invalid bearer") {
        "Invalid bearer token. Please log in again.".to_string()
    } else if lower.contains("403") || lower.contains("forbidden") || lower.contains("permission") || lower.contains("scope") {
        "Token does not have the required permissions.".to_string()
    } else if lower.contains("timeout") || lower.contains("timed out") || lower.contains("connection refused") || lower.contains("network") || lower.contains("connect") {
        "Could not reach provider. Check your connection and try again.".to_string()
    } else {
        // Truncate to 200 chars to avoid overwhelming the UI
        let truncated: String = err_str.chars().take(200).collect();
        truncated
    }
}

// ── Commands ──

/// Start an OAuth login flow for the given provider.
/// Opens the system browser and spawns a background listener for the callback.
#[tauri::command]
pub async fn start_oauth_login(
    auth: State<'_, AuthState>,
    flow: State<'_, OAuthFlowState>,
    provider: String,
) -> Result<OAuthStartResult, String> {
    // Clear prior error and mark flow as started
    flow.start(&provider)?;

    match provider.as_str() {
        "openai" => {
            let auth_clone = auth.inner().clone();
            let flow_clone = flow.inner().clone();
            start_openai_oauth(auth_clone, flow_clone).await?;
            Ok(OAuthStartResult {
                started: true,
                provider,
            })
        }
        "gemini" => {
            let auth_clone = auth.inner().clone();
            let flow_clone = flow.inner().clone();
            start_gemini_oauth(auth_clone, flow_clone).await?;
            Ok(OAuthStartResult {
                started: true,
                provider,
            })
        }
        _ => Err(format!("OAuth not supported for provider: {}", provider)),
    }
}

async fn start_openai_oauth(auth: AuthState, flow: OAuthFlowState) -> Result<(), String> {
    let pkce = generate_pkce_state();
    let url = openai_oauth::build_authorize_url(&pkce);

    open::that(&url).map_err(|e| format!("Failed to open browser: {}", e))?;

    tauri::async_runtime::spawn(async move {
        let code_result =
            openai_oauth::receive_loopback_code(&pkce.state, Duration::from_secs(120)).await;

        match code_result {
            Ok(code) => {
                let client = reqwest::Client::new();
                match openai_oauth::exchange_code_for_tokens(&client, &code, &pkce).await {
                    Ok(token_set) => {
                        // Store in zeroclaw's AuthService so openai-codex provider can use it
                        // (with automatic token refresh support)
                        let zeroclaw_auth = AuthService::new(&default_zeroclaw_dir(), false);
                        if let Err(e) = zeroclaw_auth
                            .store_openai_tokens("default", token_set.clone(), None, true)
                            .await
                        {
                            flow.set_error("openai", &map_oauth_error(&format!("Failed to store tokens: {}", e)));
                            return;
                        }

                        // Also store in our credential store for UI status tracking
                        let token = token_set.access_token.clone();
                        let mut store = auth.store.lock().unwrap();
                        store.credentials.insert(
                            "openai".to_string(),
                            ProviderCredential {
                                provider: "openai".to_string(),
                                method: AuthMethod::OAuth,
                                api_key: None,
                                oauth_token: Some(token),
                                display_name: Some("ChatGPT (Subscription)".to_string()),
                                model: Some("gpt-4o".to_string()),
                                base_url: None,
                            },
                        );
                        if store.active_provider.is_none() {
                            store.active_provider = Some("openai".to_string());
                        }
                        drop(store);
                        let _ = auth.persist();

                        flow.set_authenticated("openai");
                    }
                    Err(e) => {
                        flow.set_error("openai", &map_oauth_error(&e.to_string()));
                    }
                }
            }
            Err(e) => {
                flow.set_error("openai", &map_oauth_error(&e.to_string()));
            }
        }
    });

    Ok(())
}

fn default_zeroclaw_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".zeroclaw")
}

async fn start_gemini_oauth(auth: AuthState, flow: OAuthFlowState) -> Result<(), String> {
    let pkce = generate_pkce_state();
    let url = gemini_oauth::build_authorize_url(&pkce)
        .map_err(|e| format!("Gemini OAuth requires GEMINI_OAUTH_CLIENT_ID and GEMINI_OAUTH_CLIENT_SECRET environment variables: {}", e))?;

    open::that(&url).map_err(|e| format!("Failed to open browser: {}", e))?;

    tauri::async_runtime::spawn(async move {
        let code_result =
            gemini_oauth::receive_loopback_code(&pkce.state, Duration::from_secs(120)).await;

        match code_result {
            Ok(code) => {
                let client = reqwest::Client::new();
                match gemini_oauth::exchange_code_for_tokens(&client, &code, &pkce).await {
                    Ok(token_set) => {
                        let token = token_set.access_token.clone();
                        let mut store = auth.store.lock().unwrap();
                        store.credentials.insert(
                            "gemini".to_string(),
                            ProviderCredential {
                                provider: "gemini".to_string(),
                                method: AuthMethod::OAuth,
                                api_key: None,
                                oauth_token: Some(token),
                                display_name: Some("Gemini (OAuth)".to_string()),
                                model: Some("gemini-2.0-flash".to_string()),
                                base_url: None,
                            },
                        );
                        if store.active_provider.is_none() {
                            store.active_provider = Some("gemini".to_string());
                        }
                        drop(store);
                        let _ = auth.persist();

                        flow.set_authenticated("gemini");
                    }
                    Err(e) => {
                        flow.set_error("gemini", &map_oauth_error(&e.to_string()));
                    }
                }
            }
            Err(e) => {
                flow.set_error("gemini", &map_oauth_error(&e.to_string()));
            }
        }
    });

    Ok(())
}

/// Save a Claude setup-token (sk-ant-oat01-*) from `claude setup-token`.
/// Validates the token against the Anthropic API before storing it.
/// This is stored as an OAuth credential so anthropic_chat sends it as Bearer + anthropic-beta header.
#[tauri::command]
pub async fn save_setup_token(
    auth: State<'_, AuthState>,
    token: String,
) -> Result<OAuthStartResult, String> {
    let trimmed = token.trim().to_string();
    if trimmed.is_empty() {
        return Err("Token cannot be empty".to_string());
    }

    if !trimmed.starts_with("sk-ant-oat01-") {
        return Err(
            "Invalid token format. Setup tokens start with sk-ant-oat01-. \
             Run `claude setup-token` in your terminal to generate one."
                .to_string(),
        );
    }

    // Validate the token against the Anthropic API before storing
    validate_anthropic_token(&trimmed).await?;

    auth.store_oauth_token(
        "claude",
        &trimmed,
        Some("Claude (Subscription)"),
        Some("claude-haiku-4-5-20251001"),
    )?;

    Ok(OAuthStartResult {
        started: true,
        provider: "claude".to_string(),
    })
}

/// Validate a setup-token by making a minimal API call to Anthropic.
/// A 200 or 400 means the token is valid (400 = authenticated but request issue).
/// Only 401 (bad token) or 403 (OAuth not allowed) are real failures.
async fn validate_anthropic_token(token: &str) -> Result<(), String> {
    let client = reqwest::Client::new();
    let res = client
        .post("https://api.anthropic.com/v1/messages")
        .header("Authorization", format!("Bearer {}", token))
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(r#"{"model":"claude-haiku-4-5-20251001","max_tokens":1,"messages":[{"role":"user","content":"hi"}]}"#)
        .send()
        .await
        .map_err(|e| format!("Network error validating token: {}", e))?;

    let status = res.status().as_u16();

    // 200 = full success, 400 = token valid but request issue (fine for validation)
    if status == 200 || status == 400 {
        return Ok(());
    }

    let body = res.text().await.unwrap_or_default();

    match status {
        401 => Err(
            "Setup token is invalid or expired. Run `claude setup-token` \
             again in your terminal to generate a fresh token."
                .to_string(),
        ),
        403 if body.contains("OAuth authentication is currently not allowed") => Err(
            "Your Anthropic account does not support setup tokens. \
             Use 'API Key' instead with a key from console.anthropic.com."
                .to_string(),
        ),
        403 => Err(format!("Token rejected by Anthropic (403): {}", body)),
        _ => Err(format!("Anthropic API error ({}): {}", status, body)),
    }
}

/// Check if an OAuth flow has completed for the given provider.
/// Returns OAuthStatusResult including any error from the flow.
#[tauri::command]
pub fn check_oauth_status(
    auth: State<AuthState>,
    flow: State<OAuthFlowState>,
    provider: String,
) -> Result<OAuthStatusResult, String> {
    let (completed, _flow_authenticated, error) = flow.status(&provider)?;

    let has_credential = auth
        .get_credential(&provider)
        .map(|c| c.is_some())
        .unwrap_or(false);

    Ok(OAuthStatusResult {
        completed,
        provider,
        authenticated: has_credential,
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test 1: OAuthStatusResult serializes error field
    #[test]
    fn test_oauth_status_serializes_error() {
        let result = OAuthStatusResult {
            completed: false,
            provider: "claude".to_string(),
            authenticated: false,
            error: Some("Invalid token".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(
            json.contains("\"error\":\"Invalid token\""),
            "Expected error field in JSON, got: {}",
            json
        );
        assert!(
            json.contains("\"authenticated\""),
            "Expected authenticated field, got: {}",
            json
        );
    }

    // Test 2: error is absent when None
    #[test]
    fn test_oauth_status_omits_error_when_none() {
        let result = OAuthStatusResult {
            completed: true,
            provider: "claude".to_string(),
            authenticated: true,
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(
            !json.contains("\"error\""),
            "error key must be absent when None, got: {}",
            json
        );
    }

    // Test 3: flow error is returned in check_oauth_status equivalent
    #[test]
    fn test_flow_state_set_error_and_status() {
        let flow = OAuthFlowState::new();

        // Start a flow
        flow.start("openai").unwrap();
        let (completed, authenticated, error) = flow.status("openai").unwrap();
        assert!(!completed);
        assert!(!authenticated);
        assert!(error.is_none());

        // Record an error
        flow.set_error("openai", "Invalid bearer token. Please log in again.");
        let (completed, authenticated, error) = flow.status("openai").unwrap();
        assert!(completed, "Error sets completed = true");
        assert!(!authenticated);
        assert_eq!(error.as_deref(), Some("Invalid bearer token. Please log in again."));
    }

    // Test 4: successful flow — error is None
    #[test]
    fn test_flow_state_authenticated_clears_error() {
        let flow = OAuthFlowState::new();
        flow.start("gemini").unwrap();
        flow.set_error("gemini", "some prior error");
        flow.set_authenticated("gemini");

        let (completed, authenticated, error) = flow.status("gemini").unwrap();
        assert!(completed);
        assert!(authenticated);
        assert!(error.is_none(), "Authenticated state must clear error");
    }

    // Test 5: fresh login clears prior error
    #[test]
    fn test_flow_state_start_clears_prior_error() {
        let flow = OAuthFlowState::new();
        flow.set_error("openai", "old error");

        // Start a fresh login
        flow.start("openai").unwrap();
        let (_, _, error) = flow.status("openai").unwrap();
        assert!(error.is_none(), "start() must clear prior error, got: {:?}", error);
    }

    // Test 6: map_oauth_error maps 401 pattern
    #[test]
    fn test_map_oauth_error_401() {
        let msg = map_oauth_error("HTTP 401 Unauthorized");
        assert!(msg.contains("Invalid bearer token"), "Got: {}", msg);
    }

    // Test 7: map_oauth_error maps timeout
    #[test]
    fn test_map_oauth_error_timeout() {
        let msg = map_oauth_error("connection timed out after 30s");
        assert!(msg.contains("Could not reach"), "Got: {}", msg);
    }

    // Test 8: map_oauth_error maps 403/scope
    #[test]
    fn test_map_oauth_error_403_scope() {
        let msg = map_oauth_error("403 Forbidden: insufficient scope");
        assert!(msg.contains("required permissions"), "Got: {}", msg);
    }
}
