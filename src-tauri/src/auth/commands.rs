use crate::auth::AuthState;
use serde::{Deserialize, Serialize};
use tauri::State;

// ── Ollama connection probe ───────────────────────────────────────────────────

/// Result returned across the IPC boundary by `check_ollama_connection`.
///
/// `connected: false` covers all "not reachable" cases (timeout, wrong scheme,
/// non-200 response, JSON parse failure). `error` carries a human-readable
/// description so the UI can surface it. `models` is populated on success from
/// `GET {base}/api/tags` and `version` from `GET {base}/api/version` (best-effort).
///
/// Security: scheme is validated before any request (http/https only). Timeout
/// is short (3 s). Redirects are not followed. No credentials are sent.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaConnectionStatus {
    pub connected: bool,
    pub models: Vec<String>,
    pub version: Option<String>,
    pub error: Option<String>,
}

/// Resolve the base URL to probe:
/// 1. `base_url` arg (if Some and non-empty after trimming)
/// 2. Stored ollama credential `base_url`
/// 3. Default `http://localhost:11434`
///
/// Returns `(resolved_url, trailing_slash_stripped)`.
pub fn resolve_ollama_base_url(
    arg: Option<&str>,
    auth: &AuthState,
) -> String {
    // Priority 1: explicit arg
    if let Some(u) = arg {
        let trimmed = u.trim();
        if !trimmed.is_empty() {
            return trimmed.trim_end_matches('/').to_string();
        }
    }

    // Priority 2: stored credential
    if let Ok(Some(cred)) = auth.get_credential("ollama") {
        if let Some(stored) = cred.base_url {
            let trimmed = stored.trim().trim_end_matches('/').to_string();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
    }

    // Priority 3: default
    "http://localhost:11434".to_string()
}

/// Return `true` iff `url` starts with `http://` or `https://` (case-insensitive).
pub fn is_valid_scheme(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Probe Ollama connectivity at `base_url`.
///
/// - Validates the scheme (http/https only); returns `connected:false` on anything else.
/// - `GET {base}/api/tags` with 3 s timeout, no redirects.
/// - On HTTP 200 with parseable JSON `{ "models": [...] }`, returns `connected:true`.
/// - On any failure (wrong scheme, connection refused, timeout, non-200, bad JSON)
///   returns `Ok(OllamaConnectionStatus { connected: false, … })` — NEVER panics,
///   NEVER returns `Err` for the normal "not reachable" case.
/// - Attempts `GET {base}/api/version` for a version string only when tags succeeds
///   (best-effort; omitted on any sub-error).
#[tauri::command]
pub async fn check_ollama_connection(
    auth: State<'_, AuthState>,
    base_url: Option<String>,
) -> Result<OllamaConnectionStatus, String> {
    let base = resolve_ollama_base_url(base_url.as_deref(), &auth);

    // Scheme guard — reject anything other than http/https (mild SSRF defence).
    if !is_valid_scheme(&base) {
        return Ok(OllamaConnectionStatus {
            connected: false,
            models: vec![],
            version: None,
            error: Some(format!(
                "Unsupported scheme in '{}' — only http and https are allowed",
                base
            )),
        });
    }

    // Build a one-shot client: 3 s timeout, no redirect following.
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            // Truly unexpected internal failure — safe to propagate as Err.
            return Err(format!("Failed to build HTTP client: {}", e));
        }
    };

    // Probe: GET {base}/api/tags
    let tags_url = format!("{}/api/tags", base);
    let resp = match client.get(&tags_url).send().await {
        Ok(r) => r,
        Err(e) => {
            return Ok(OllamaConnectionStatus {
                connected: false,
                models: vec![],
                version: None,
                error: Some(format!("Ollama not reachable at {}: {}", base, e)),
            });
        }
    };

    if resp.status().as_u16() != 200 {
        return Ok(OllamaConnectionStatus {
            connected: false,
            models: vec![],
            version: None,
            error: Some(format!(
                "Ollama at {} returned HTTP {}",
                base,
                resp.status().as_u16()
            )),
        });
    }

    // Parse JSON: expect `{ "models": [{ "name": "..." }, ...] }`
    let body = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            return Ok(OllamaConnectionStatus {
                connected: false,
                models: vec![],
                version: None,
                error: Some(format!("Failed to read Ollama response body: {}", e)),
            });
        }
    };

    let json: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            return Ok(OllamaConnectionStatus {
                connected: false,
                models: vec![],
                version: None,
                error: Some(format!("Ollama response is not valid JSON: {}", e)),
            });
        }
    };

    // Extract model names from `{ "models": [{ "name": "llama3" }, ...] }`.
    let models: Vec<String> = json["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // Best-effort: fetch version string from GET {base}/api/version.
    // Errors here do NOT affect connected:true — just omit version.
    let version: Option<String> = match client.get(format!("{}/api/version", base)).send().await {
        Ok(vr) if vr.status().as_u16() == 200 => vr
            .text()
            .await
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| v["version"].as_str().map(|s| s.to_string())),
        _ => None,
    };

    Ok(OllamaConnectionStatus {
        connected: true,
        models,
        version,
        error: None,
    })
}

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

/// Report which providers are already configured in the credential store.
/// Does NOT auto-import from environment variables — BYOK must be explicit.
#[tauri::command]
pub fn detect_system_providers(auth: State<AuthState>) -> Result<Vec<DetectedProvider>, String> {
    let mut detected = Vec::new();
    let providers = ["claude", "openai", "gemini", "ollama"];

    for provider in providers {
        if let Ok(Some(cred)) = auth.get_credential(provider) {
            detected.push(DetectedProvider {
                provider: provider.to_string(),
                source: format!("{:?}", cred.method).to_lowercase(),
                imported: false,
            });
        }
    }

    Ok(detected)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_auth() -> (AuthState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let state = AuthState::new(&dir.path().to_path_buf());
        (state, dir)
    }

    // ── resolve_ollama_base_url ──────────────────────────────────────────────

    #[test]
    fn resolve_uses_explicit_arg_when_provided() {
        let (auth, _dir) = temp_auth();
        let url = resolve_ollama_base_url(Some("http://custom:11434"), &auth);
        assert_eq!(url, "http://custom:11434");
    }

    #[test]
    fn resolve_strips_trailing_slash_from_arg() {
        let (auth, _dir) = temp_auth();
        let url = resolve_ollama_base_url(Some("http://custom:11434/"), &auth);
        assert_eq!(url, "http://custom:11434");
    }

    #[test]
    fn resolve_falls_back_to_stored_credential_when_arg_is_none() {
        let (auth, _dir) = temp_auth();
        auth.store_ollama_config("http://stored:11435", None).unwrap();
        let url = resolve_ollama_base_url(None, &auth);
        assert_eq!(url, "http://stored:11435");
    }

    #[test]
    fn resolve_falls_back_to_stored_credential_when_arg_is_empty() {
        let (auth, _dir) = temp_auth();
        auth.store_ollama_config("http://stored:11435", None).unwrap();
        let url = resolve_ollama_base_url(Some(""), &auth);
        assert_eq!(url, "http://stored:11435");
    }

    #[test]
    fn resolve_strips_trailing_slash_from_stored_credential() {
        let (auth, _dir) = temp_auth();
        auth.store_ollama_config("http://stored:11435/", None).unwrap();
        let url = resolve_ollama_base_url(None, &auth);
        assert_eq!(url, "http://stored:11435");
    }

    #[test]
    fn resolve_defaults_to_localhost_when_no_arg_and_no_credential() {
        let (auth, _dir) = temp_auth();
        let url = resolve_ollama_base_url(None, &auth);
        assert_eq!(url, "http://localhost:11434");
    }

    #[test]
    fn resolve_arg_takes_priority_over_stored_credential() {
        let (auth, _dir) = temp_auth();
        auth.store_ollama_config("http://stored:11435", None).unwrap();
        let url = resolve_ollama_base_url(Some("http://explicit:9999"), &auth);
        assert_eq!(url, "http://explicit:9999");
    }

    // ── is_valid_scheme ──────────────────────────────────────────────────────

    #[test]
    fn valid_scheme_http_accepted() {
        assert!(is_valid_scheme("http://localhost:11434"));
    }

    #[test]
    fn valid_scheme_https_accepted() {
        assert!(is_valid_scheme("https://example.com:11434"));
    }

    #[test]
    fn invalid_scheme_file_rejected() {
        assert!(!is_valid_scheme("file:///etc/passwd"));
    }

    #[test]
    fn invalid_scheme_ftp_rejected() {
        assert!(!is_valid_scheme("ftp://somehost/"));
    }

    #[test]
    fn invalid_scheme_empty_rejected() {
        assert!(!is_valid_scheme(""));
    }

    #[test]
    fn invalid_scheme_no_scheme_rejected() {
        assert!(!is_valid_scheme("localhost:11434"));
    }

    #[test]
    fn invalid_scheme_uppercase_http_still_accepted() {
        // scheme check must be case-insensitive
        assert!(is_valid_scheme("HTTP://localhost:11434"));
        assert!(is_valid_scheme("HTTPS://localhost:11434"));
    }

    // ── Network test (gated behind LEARNFORGE_TEST_OLLAMA=1) ─────────────────
    //
    // This test requires a live Ollama instance at http://localhost:11434.
    // It is SKIPPED by default so that `cargo test --lib` does NOT depend on
    // a running daemon. To run it:
    //   LEARNFORGE_TEST_OLLAMA=1 cargo test --lib -- check_ollama_live
    #[tokio::test]
    async fn check_ollama_live() {
        if std::env::var("LEARNFORGE_TEST_OLLAMA").as_deref() != Ok("1") {
            return; // skip
        }
        let dir = tempfile::tempdir().unwrap();
        let auth = AuthState::new(&dir.path().to_path_buf());
        let base = resolve_ollama_base_url(None, &auth); // → http://localhost:11434

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();

        let resp = client.get(format!("{}/api/tags", base)).send().await;
        assert!(resp.is_ok(), "live Ollama must be reachable");
        let r = resp.unwrap();
        assert_eq!(r.status().as_u16(), 200, "live Ollama must return 200 on /api/tags");
        let body = r.text().await.unwrap();
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["models"].is_array(), "live Ollama response must have a 'models' array");
        let models: Vec<String> = json["models"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
            .collect();
        assert!(!models.is_empty(), "live Ollama must have at least one model loaded");
    }
}
