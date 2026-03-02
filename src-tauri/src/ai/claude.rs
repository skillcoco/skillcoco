use super::provider::AIProvider;

pub struct ClaudeProvider {
    api_key: String,
    model: String,
}

impl ClaudeProvider {
    pub fn new(api_key: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }
}

impl AIProvider for ClaudeProvider {
    fn name(&self) -> &str {
        "claude"
    }
}

// TODO: Implement actual Anthropic API calls
// - POST https://api.anthropic.com/v1/messages
// - Headers: x-api-key, anthropic-version, content-type
// - Body: model, max_tokens, system, messages[]
