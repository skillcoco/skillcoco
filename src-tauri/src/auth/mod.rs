pub mod commands;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

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
struct CredentialStore {
    active_provider: Option<String>,
    credentials: HashMap<String, ProviderCredential>,
}

pub struct AuthState {
    store_path: PathBuf,
    store: Mutex<CredentialStore>,
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
            store: Mutex::new(store),
        }
    }

    fn persist(&self) -> Result<(), String> {
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
