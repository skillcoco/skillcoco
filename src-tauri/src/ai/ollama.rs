use super::provider::AIProvider;

pub struct OllamaProvider {
    base_url: String,
    model: String,
}

impl OllamaProvider {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            model: model.to_string(),
        }
    }
}

impl AIProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }
}

// TODO: Implement Ollama API calls
// - POST {base_url}/api/chat
