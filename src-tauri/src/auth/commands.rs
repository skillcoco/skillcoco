use crate::auth::AuthState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAuthStatus {
    pub provider: String,
    pub authenticated: bool,
    pub method: String,
    pub display_name: Option<String>,
    pub model: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedProvider {
    pub provider: String,
    pub source: String,
    pub imported: bool,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub provider: String,
    pub method: String,   // "api-key" | "ollama"
    pub credential: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

/// Get authentication status for all supported providers.
#[tauri::command]
pub fn get_auth_status(auth: State<AuthState>) -> Result<Vec<ProviderAuthStatus>, String> {
    let providers = ["claude", "openai", "gemini", "ollama"];
    let active = auth.get_active_provider()?;
    let mut statuses = Vec::new();

    for provider in providers {
        let cred = auth.get_credential(provider)?;
        let (authenticated, method, display_name, model) = match &cred {
            Some(c) => (
                true,
                format!("{:?}", c.method).to_lowercase(),
                c.display_name.clone(),
                c.model.clone(),
            ),
            None => (false, "none".to_string(), None, None),
        };

        statuses.push(ProviderAuthStatus {
            provider: provider.to_string(),
            authenticated,
            method,
            display_name,
            model,
            is_active: active.as_deref() == Some(provider),
        });
    }

    Ok(statuses)
}

/// Store credentials for a provider (API key or Ollama config).
#[tauri::command]
pub fn login_provider(auth: State<AuthState>, request: LoginRequest) -> Result<ProviderAuthStatus, String> {
    match request.method.as_str() {
        "api-key" => {
            let key = request.credential.ok_or("API key is required")?;
            auth.store_api_key(&request.provider, &key, request.model.as_deref())?;
        }
        "ollama" => {
            let base_url = request.base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
            auth.store_ollama_config(&base_url, request.model.as_deref())?;
        }
        other => return Err(format!("Unsupported auth method: {}", other)),
    }

    let active = auth.get_active_provider()?;

    Ok(ProviderAuthStatus {
        provider: request.provider.clone(),
        authenticated: true,
        method: request.method,
        display_name: None,
        model: request.model,
        is_active: active.as_deref() == Some(request.provider.as_str()),
    })
}

/// Set the active AI provider.
#[tauri::command]
pub fn set_active_provider(auth: State<AuthState>, provider: String) -> Result<(), String> {
    auth.set_active_provider(&provider)
}

/// Remove credentials for a provider.
#[tauri::command]
pub fn logout_provider(auth: State<AuthState>, provider: String) -> Result<(), String> {
    auth.remove_credential(&provider)
}

/// Detect API keys from environment variables and register providers.
/// Checks ANTHROPIC_API_KEY, OPENAI_API_KEY, and GEMINI_API_KEY.
/// Only imports if no credential is stored yet (subscription tokens take priority).
/// Also checks ANTHROPIC_OAUTH_TOKEN for Claude setup-token users.
#[tauri::command]
pub fn detect_system_providers(auth: State<AuthState>) -> Result<Vec<DetectedProvider>, String> {
    let mut detected = Vec::new();

    // Check for subscription tokens first (setup-token, etc.)
    if let Ok(token) = std::env::var("ANTHROPIC_OAUTH_TOKEN") {
        let trimmed = token.trim().to_string();
        if !trimmed.is_empty() {
            let already_has = auth.get_credential("claude")?.is_some();
            if !already_has {
                auth.store_oauth_token("claude", &trimmed, Some("Claude (subscription)"), Some("claude-sonnet-4-20250514"))?;
            }
            detected.push(DetectedProvider {
                provider: "claude".to_string(),
                source: "ANTHROPIC_OAUTH_TOKEN".to_string(),
                imported: !already_has,
            });
        }
    }

    // Then check API keys as fallback
    let checks: &[(&str, &str, &str, &str)] = &[
        ("ANTHROPIC_API_KEY", "claude", "claude-sonnet-4-20250514", "ANTHROPIC_API_KEY"),
        ("OPENAI_API_KEY", "openai", "gpt-4o", "OPENAI_API_KEY"),
        ("GEMINI_API_KEY", "gemini", "gemini-2.0-flash", "GEMINI_API_KEY"),
    ];

    for (env_var, provider, model, source_label) in checks {
        if let Ok(key) = std::env::var(env_var) {
            let trimmed = key.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }
            let already_has = auth.get_credential(provider)?.is_some();
            if !already_has {
                auth.store_api_key(provider, &trimmed, Some(model))?;
            }
            detected.push(DetectedProvider {
                provider: provider.to_string(),
                source: source_label.to_string(),
                imported: !already_has,
            });
        }
    }

    Ok(detected)
}
