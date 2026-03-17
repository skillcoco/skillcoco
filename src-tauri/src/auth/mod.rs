pub mod commands;
pub mod oauth;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Supported auth methods for AI providers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum AuthMethod {
    OAuth,
    ApiKey,
    None, // Ollama / local
}

/// Per-provider credential storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCredential {
    pub provider: String,
    pub method: AuthMethod,
    pub api_key: Option<String>,
    pub oauth_token: Option<String>,
    pub display_name: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

/// Persistent credential store backed by a JSON file.
/// Credentials are stored in the Tauri app data directory.
#[derive(Debug, Serialize, Deserialize, Default)]
pub(crate) struct CredentialStore {
    pub(crate) active_provider: Option<String>,
    pub(crate) credentials: HashMap<String, ProviderCredential>,
}

#[derive(Clone)]
pub struct AuthState {
    pub(crate) store_path: PathBuf,
    pub(crate) store: Arc<Mutex<CredentialStore>>,
}

impl AuthState {
    pub fn new(state_dir: &PathBuf) -> Self {
        let store_path = state_dir.join("credentials.json");
        let store = if store_path.exists() {
            match std::fs::read_to_string(&store_path) {
                Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
                Err(_) => CredentialStore::default(),
            }
        } else {
            CredentialStore::default()
        };

        Self {
            store_path,
            store: Arc::new(Mutex::new(store)),
        }
    }

    pub(crate) fn persist(&self) -> Result<(), String> {
        let store = self.store.lock().map_err(|e| e.to_string())?;
        let data = serde_json::to_string_pretty(&*store).map_err(|e| e.to_string())?;
        std::fs::write(&self.store_path, data).map_err(|e| e.to_string())
    }

    pub fn store_api_key(
        &self,
        provider: &str,
        api_key: &str,
        model: Option<&str>,
    ) -> Result<(), String> {
        let mut store = self.store.lock().map_err(|e| e.to_string())?;
        store.credentials.insert(
            provider.to_string(),
            ProviderCredential {
                provider: provider.to_string(),
                method: AuthMethod::ApiKey,
                api_key: Some(api_key.to_string()),
                oauth_token: None,
                display_name: None,
                model: model.map(String::from),
                base_url: None,
            },
        );
        if store.active_provider.is_none() {
            store.active_provider = Some(provider.to_string());
        }
        drop(store);
        self.persist()
    }

    pub fn store_ollama_config(
        &self,
        base_url: &str,
        model: Option<&str>,
    ) -> Result<(), String> {
        let mut store = self.store.lock().map_err(|e| e.to_string())?;
        store.credentials.insert(
            "ollama".to_string(),
            ProviderCredential {
                provider: "ollama".to_string(),
                method: AuthMethod::None,
                api_key: None,
                oauth_token: None,
                display_name: Some("Ollama (Local)".to_string()),
                model: model.map(String::from),
                base_url: Some(base_url.to_string()),
            },
        );
        drop(store);
        self.persist()
    }

    /// Store a setup-token (Claude) or OAuth token for a provider.
    /// The token is stored in oauth_token and method is set to OAuth.
    pub fn store_oauth_token(
        &self,
        provider: &str,
        token: &str,
        display_name: Option<&str>,
        model: Option<&str>,
    ) -> Result<(), String> {
        let mut store = self.store.lock().map_err(|e| e.to_string())?;
        store.credentials.insert(
            provider.to_string(),
            ProviderCredential {
                provider: provider.to_string(),
                method: AuthMethod::OAuth,
                api_key: None,
                oauth_token: Some(token.to_string()),
                display_name: display_name.map(String::from),
                model: model.map(String::from),
                base_url: None,
            },
        );
        if store.active_provider.is_none() {
            store.active_provider = Some(provider.to_string());
        }
        drop(store);
        self.persist()
    }

    pub fn get_credential(&self, provider: &str) -> Result<Option<ProviderCredential>, String> {
        let store = self.store.lock().map_err(|e| e.to_string())?;
        Ok(store.credentials.get(provider).cloned())
    }

    pub fn get_active_provider(&self) -> Result<Option<String>, String> {
        let store = self.store.lock().map_err(|e| e.to_string())?;
        Ok(store.active_provider.clone())
    }

    pub fn set_active_provider(&self, provider: &str) -> Result<(), String> {
        let mut store = self.store.lock().map_err(|e| e.to_string())?;
        if !store.credentials.contains_key(provider) {
            return Err(format!("No credentials stored for provider: {}", provider));
        }
        store.active_provider = Some(provider.to_string());
        drop(store);
        self.persist()
    }

    pub fn remove_credential(&self, provider: &str) -> Result<(), String> {
        let mut store = self.store.lock().map_err(|e| e.to_string())?;
        store.credentials.remove(provider);
        if store.active_provider.as_deref() == Some(provider) {
            store.active_provider = store.credentials.keys().next().cloned();
        }
        drop(store);
        self.persist()
    }

    pub fn list_credentials(&self) -> Result<Vec<ProviderCredential>, String> {
        let store = self.store.lock().map_err(|e| e.to_string())?;
        Ok(store.credentials.values().cloned().collect())
    }

    /// Get the active credential, resolving to the active provider.
    pub fn get_active_credential(&self) -> Result<Option<ProviderCredential>, String> {
        let store = self.store.lock().map_err(|e| e.to_string())?;
        let active = match &store.active_provider {
            Some(p) => p,
            None => return Ok(None),
        };
        Ok(store.credentials.get(active).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_auth_state() -> (AuthState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let state = AuthState::new(&dir.path().to_path_buf());
        (state, dir)
    }

    #[test]
    fn test_new_creates_empty_store() {
        let (state, _dir) = temp_auth_state();
        assert!(state.get_active_provider().unwrap().is_none());
        assert!(state.list_credentials().unwrap().is_empty());
    }

    #[test]
    fn test_store_api_key() {
        let (state, _dir) = temp_auth_state();
        state.store_api_key("anthropic", "sk-test-123", Some("claude-sonnet-4-20250514")).unwrap();

        let cred = state.get_credential("anthropic").unwrap().unwrap();
        assert_eq!(cred.provider, "anthropic");
        assert_eq!(cred.method, AuthMethod::ApiKey);
        assert_eq!(cred.api_key.as_deref(), Some("sk-test-123"));
        assert_eq!(cred.model.as_deref(), Some("claude-sonnet-4-20250514"));
    }

    #[test]
    fn test_first_stored_becomes_active() {
        let (state, _dir) = temp_auth_state();
        state.store_api_key("openai", "sk-openai", None).unwrap();

        assert_eq!(state.get_active_provider().unwrap().as_deref(), Some("openai"));
    }

    #[test]
    fn test_second_stored_does_not_change_active() {
        let (state, _dir) = temp_auth_state();
        state.store_api_key("openai", "sk-openai", None).unwrap();
        state.store_api_key("anthropic", "sk-ant", None).unwrap();

        assert_eq!(state.get_active_provider().unwrap().as_deref(), Some("openai"));
    }

    #[test]
    fn test_set_active_provider() {
        let (state, _dir) = temp_auth_state();
        state.store_api_key("openai", "sk-openai", None).unwrap();
        state.store_api_key("anthropic", "sk-ant", None).unwrap();
        state.set_active_provider("anthropic").unwrap();

        assert_eq!(state.get_active_provider().unwrap().as_deref(), Some("anthropic"));
    }

    #[test]
    fn test_set_active_provider_unknown_fails() {
        let (state, _dir) = temp_auth_state();
        let result = state.set_active_provider("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_store_ollama_config() {
        let (state, _dir) = temp_auth_state();
        state.store_ollama_config("http://localhost:11434", Some("llama3")).unwrap();

        let cred = state.get_credential("ollama").unwrap().unwrap();
        assert_eq!(cred.method, AuthMethod::None);
        assert_eq!(cred.base_url.as_deref(), Some("http://localhost:11434"));
        assert_eq!(cred.model.as_deref(), Some("llama3"));
        assert!(cred.api_key.is_none());
    }

    #[test]
    fn test_remove_credential() {
        let (state, _dir) = temp_auth_state();
        state.store_api_key("openai", "sk-openai", None).unwrap();
        state.remove_credential("openai").unwrap();

        assert!(state.get_credential("openai").unwrap().is_none());
        assert!(state.list_credentials().unwrap().is_empty());
    }

    #[test]
    fn test_remove_active_falls_back() {
        let (state, _dir) = temp_auth_state();
        state.store_api_key("openai", "sk-openai", None).unwrap();
        state.store_api_key("anthropic", "sk-ant", None).unwrap();
        state.remove_credential("openai").unwrap();

        // Active should fall back to remaining provider
        let active = state.get_active_provider().unwrap();
        assert!(active.is_some());
        assert_eq!(active.as_deref(), Some("anthropic"));
    }

    #[test]
    fn test_get_active_credential() {
        let (state, _dir) = temp_auth_state();
        state.store_api_key("anthropic", "sk-ant-123", None).unwrap();

        let cred = state.get_active_credential().unwrap().unwrap();
        assert_eq!(cred.provider, "anthropic");
        assert_eq!(cred.api_key.as_deref(), Some("sk-ant-123"));
    }

    #[test]
    fn test_get_active_credential_none_when_empty() {
        let (state, _dir) = temp_auth_state();
        assert!(state.get_active_credential().unwrap().is_none());
    }

    #[test]
    fn test_persistence_across_instances() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();

        // Store credential
        {
            let state = AuthState::new(&path);
            state.store_api_key("anthropic", "sk-persist", Some("claude-sonnet-4-20250514")).unwrap();
        }

        // Load from same path
        {
            let state = AuthState::new(&path);
            let cred = state.get_credential("anthropic").unwrap().unwrap();
            assert_eq!(cred.api_key.as_deref(), Some("sk-persist"));
            assert_eq!(state.get_active_provider().unwrap().as_deref(), Some("anthropic"));
        }
    }

    #[test]
    fn test_list_credentials() {
        let (state, _dir) = temp_auth_state();
        state.store_api_key("openai", "sk-1", None).unwrap();
        state.store_api_key("anthropic", "sk-2", None).unwrap();
        state.store_ollama_config("http://localhost:11434", None).unwrap();

        let creds = state.list_credentials().unwrap();
        assert_eq!(creds.len(), 3);
    }

    #[test]
    fn test_corrupt_file_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let cred_path = dir.path().join("credentials.json");
        fs::write(&cred_path, "not valid json").unwrap();

        let state = AuthState::new(&dir.path().to_path_buf());
        assert!(state.list_credentials().unwrap().is_empty());
    }
}
