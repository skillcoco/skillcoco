use crate::auth::AuthState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderAuthStatus {
    pub provider: String,
    pub authenticated: bool,
    pub method: String,
    pub display_name: Option<String>,
    pub model: Option<String>,
    pub is_active: bool,
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
    let providers = ["anthropic", "openai", "gemini", "ollama"];
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
