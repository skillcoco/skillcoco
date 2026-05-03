use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learner_profile_serializes_camel_case() {
        let profile = LearnerProfile {
            id: "p1".to_string(),
            display_name: "Alice".to_string(),
            learning_style: "visual".to_string(),
            experience_level: "intermediate".to_string(),
            preferences_json: "{}".to_string(),
            created_at: "2026-01-01".to_string(),
            updated_at: "2026-01-01".to_string(),
        };
        let json = serde_json::to_string(&profile).unwrap();
        assert!(json.contains("\"displayName\""), "Expected displayName in JSON, got: {}", json);
        assert!(json.contains("\"learningStyle\""), "Expected learningStyle in JSON, got: {}", json);
        assert!(json.contains("\"experienceLevel\""), "Expected experienceLevel in JSON, got: {}", json);
        assert!(json.contains("\"preferencesJson\""), "Expected preferencesJson in JSON, got: {}", json);
        assert!(json.contains("\"createdAt\""), "Expected createdAt in JSON, got: {}", json);
        assert!(json.contains("\"updatedAt\""), "Expected updatedAt in JSON, got: {}", json);
        assert!(!json.contains("\"display_name\""), "Snake case display_name must not appear");
    }

    #[test]
    fn test_learning_track_round_trip_camel_case() {
        let track = LearningTrack {
            id: "t1".to_string(),
            learner_id: "p1".to_string(),
            topic: "Kubernetes".to_string(),
            domain_module: "devops".to_string(),
            status: "active".to_string(),
            goal: "Pass CKA".to_string(),
            current_module_id: Some("m1".to_string()),
            progress_percent: 42.5,
            total_time_spent: 3600,
            created_at: "2026-01-01".to_string(),
            updated_at: "2026-01-01".to_string(),
        };
        let json = serde_json::to_string(&track).unwrap();
        assert!(json.contains("\"learnerId\""), "Expected learnerId, got: {}", json);
        assert!(json.contains("\"domainModule\""), "Expected domainModule, got: {}", json);
        assert!(json.contains("\"currentModuleId\""), "Expected currentModuleId, got: {}", json);
        assert!(json.contains("\"progressPercent\""), "Expected progressPercent, got: {}", json);
        assert!(json.contains("\"totalTimeSpent\""), "Expected totalTimeSpent, got: {}", json);
        // Round-trip
        let decoded: LearningTrack = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.learner_id, "p1");
        assert_eq!(decoded.domain_module, "devops");
        assert_eq!(decoded.current_module_id, Some("m1".to_string()));
        assert_eq!(decoded.progress_percent, 42.5);
        assert_eq!(decoded.total_time_spent, 3600);
    }

    #[test]
    fn test_learning_path_serializes_camel_case() {
        let path = LearningPath {
            id: "path-1".to_string(),
            track_id: "t1".to_string(),
            version: 1,
            generated_by_model: "claude-sonnet-4-6".to_string(),
            modules_json: "[]".to_string(),
            edges_json: "[]".to_string(),
            estimated_hours: 5.0,
            created_at: "2026-01-01".to_string(),
        };
        let json = serde_json::to_string(&path).unwrap();
        assert!(json.contains("\"trackId\""), "Expected trackId, got: {}", json);
        assert!(json.contains("\"generatedByModel\""), "Expected generatedByModel, got: {}", json);
        assert!(json.contains("\"modulesJson\""), "Expected modulesJson, got: {}", json);
        assert!(json.contains("\"edgesJson\""), "Expected edgesJson, got: {}", json);
        assert!(json.contains("\"estimatedHours\""), "Expected estimatedHours, got: {}", json);
        assert!(json.contains("\"createdAt\""), "Expected createdAt, got: {}", json);
    }

    #[test]
    fn test_module_progress_serializes_camel_case() {
        let mp = ModuleProgress {
            id: "mp-1".to_string(),
            module_id: "m1".to_string(),
            learner_id: "p1".to_string(),
            status: "in_progress".to_string(),
            score: Some(0.85),
            time_spent: 1200,
            attempts: 2,
            mastery_level: 0.65,
            started_at: Some("2026-01-01".to_string()),
            completed_at: None,
        };
        let json = serde_json::to_string(&mp).unwrap();
        assert!(json.contains("\"moduleId\""), "Expected moduleId, got: {}", json);
        assert!(json.contains("\"learnerId\""), "Expected learnerId, got: {}", json);
        assert!(json.contains("\"masteryLevel\""), "Expected masteryLevel, got: {}", json);
        assert!(json.contains("\"timeSpent\""), "Expected timeSpent, got: {}", json);
        assert!(json.contains("\"startedAt\""), "Expected startedAt, got: {}", json);
        assert!(json.contains("\"completedAt\""), "Expected completedAt, got: {}", json);
    }

    #[test]
    fn test_sr_card_serializes_camel_case() {
        let card = SRCard {
            id: "c1".to_string(),
            module_id: "m1".to_string(),
            concept: "Pods".to_string(),
            card_type: "active_recall".to_string(),
            front: "What is a Pod?".to_string(),
            back: "Smallest deployable unit".to_string(),
            interval_days: 3.0,
            ease_factor: 2.5,
            repetitions: 2,
            next_review: "2026-01-04".to_string(),
            last_review: Some("2026-01-01".to_string()),
        };
        let json = serde_json::to_string(&card).unwrap();
        assert!(json.contains("\"moduleId\""), "Expected moduleId, got: {}", json);
        assert!(json.contains("\"cardType\""), "Expected cardType, got: {}", json);
        assert!(json.contains("\"intervalDays\""), "Expected intervalDays, got: {}", json);
        assert!(json.contains("\"easeFactor\""), "Expected easeFactor, got: {}", json);
        assert!(json.contains("\"nextReview\""), "Expected nextReview, got: {}", json);
        assert!(json.contains("\"lastReview\""), "Expected lastReview, got: {}", json);
    }
}
