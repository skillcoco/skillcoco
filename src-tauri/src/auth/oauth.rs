use crate::auth::{AuthMethod, AuthState, ProviderCredential};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::State;
use zeroclaw::auth::oauth_common::generate_pkce_state;
use zeroclaw::auth::{gemini_oauth, openai_oauth};

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
                        let token = token_set.access_token.clone();
                        let mut store = auth.store.lock().unwrap();
                        store.credentials.insert(
                            "openai".to_string(),
                            ProviderCredential {
                                provider: "openai".to_string(),
                                method: AuthMethod::OAuth,
                                api_key: None,
                                oauth_token: Some(token),
                                display_name: Some("OpenAI (OAuth)".to_string()),
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
/// This is stored as an OAuth credential so zeroclaw sends it as Bearer + anthropic-beta header.
#[tauri::command]
pub fn save_setup_token(
    auth: State<AuthState>,
    token: String,
) -> Result<OAuthStartResult, String> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err("Token cannot be empty".to_string());
    }

    auth.store_oauth_token(
        "claude",
        trimmed,
        Some("Claude (Setup Token)"),
        Some("claude-sonnet-4-20250514"),
    )?;

    Ok(OAuthStartResult {
        started: true,
        provider: "claude".to_string(),
    })
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
