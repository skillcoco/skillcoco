use crate::auth::{AuthMethod, AuthState, ProviderCredential};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::State;
use zeroclaw::auth::oauth_common::generate_pkce_state;
use zeroclaw::auth::{gemini_oauth, openai_oauth, AuthService};

/// Tracks in-flight OAuth flows.
#[derive(Clone)]
pub struct OAuthFlowState {
    pub completed: Arc<Mutex<HashMap<String, bool>>>,
}

impl OAuthFlowState {
    pub fn new() -> Self {
        Self {
            completed: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct OAuthStartResult {
    pub started: bool,
    pub provider: String,
}

#[derive(Debug, Serialize)]
pub struct OAuthStatusResult {
    pub completed: bool,
    pub provider: String,
    pub authenticated: bool,
}

/// Start an OAuth login flow for the given provider.
/// Opens the system browser and spawns a background listener for the callback.
#[tauri::command]
pub async fn start_oauth_login(
    auth: State<'_, AuthState>,
    flow: State<'_, OAuthFlowState>,
    provider: String,
) -> Result<OAuthStartResult, String> {
    // Mark flow as not completed
    {
        let mut completed = flow.completed.lock().map_err(|e| e.to_string())?;
        completed.insert(provider.clone(), false);
    }

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
                            eprintln!("Failed to store tokens in zeroclaw AuthService: {}", e);
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

                        if let Ok(mut completed) = flow.completed.lock() {
                            completed.insert("openai".to_string(), true);
                        }
                    }
                    Err(e) => {
                        eprintln!("OAuth token exchange failed: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("OAuth callback failed: {}", e);
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

                        if let Ok(mut completed) = flow.completed.lock() {
                            completed.insert("gemini".to_string(), true);
                        }
                    }
                    Err(e) => {
                        eprintln!("Gemini OAuth token exchange failed: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Gemini OAuth callback failed: {}", e);
            }
        }
    });

    Ok(())
}

/// Save a Claude setup-token (sk-ant-oat01-*) from `claude setup-token`.
/// Validates the token against the Anthropic API before storing it.
/// This is stored as an OAuth credential so zeroclaw sends it as Bearer + anthropic-beta header.
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
        .body(r#"{"model":"claude-sonnet-4-6","max_tokens":1,"messages":[{"role":"user","content":"hi"}]}"#)
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
#[tauri::command]
pub fn check_oauth_status(
    auth: State<AuthState>,
    flow: State<OAuthFlowState>,
    provider: String,
) -> Result<OAuthStatusResult, String> {
    let completed = flow.completed.lock().map_err(|e| e.to_string())?;
    let is_completed = completed.get(&provider).copied().unwrap_or(false);

    let has_credential = auth
        .get_credential(&provider)
        .map(|c| c.is_some())
        .unwrap_or(false);

    Ok(OAuthStatusResult {
        completed: is_completed,
        provider,
        authenticated: has_credential,
    })
}
