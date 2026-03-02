use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LearnerProfile {
    pub id: String,
    pub display_name: String,
    pub learning_style: String,
    pub experience_level: String,
    pub preferences_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LearningTrack {
    pub id: String,
    pub learner_id: String,
    pub topic: String,
    pub domain_module: String,
    pub status: String,
    pub goal: String,
    pub current_module_id: Option<String>,
    pub progress_percent: f64,
    pub total_time_spent: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LearningPath {
    pub id: String,
    pub track_id: String,
    pub version: i32,
    pub generated_by_model: String,
    pub modules_json: String,
    pub edges_json: String,
    pub estimated_hours: f64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModuleProgress {
    pub id: String,
    pub module_id: String,
    pub learner_id: String,
    pub status: String,
    pub score: Option<f64>,
    pub time_spent: i64,
    pub attempts: i32,
    pub mastery_level: f64,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SRCard {
    pub id: String,
    pub module_id: String,
    pub concept: String,
    pub card_type: String,
    pub front: String,
    pub back: String,
    pub interval_days: f64,
    pub ease_factor: f64,
    pub repetitions: i32,
    pub next_review: String,
    pub last_review: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AIConfig {
    pub provider_type: String,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub max_tokens: i32,
    pub temperature: f64,
}
