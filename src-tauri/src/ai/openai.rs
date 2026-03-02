use super::provider::AIProvider;

pub struct OpenAIProvider {
    api_key: String,
    model: String,
}

impl OpenAIProvider {
    pub fn new(api_key: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }
}

impl AIProvider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }
}

// TODO: Implement OpenAI API calls
// - POST https://api.openai.com/v1/chat/completions
