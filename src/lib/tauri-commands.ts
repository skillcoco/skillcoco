/**
 * Typed wrappers around Tauri IPC invoke() calls.
 * These map 1:1 to Rust #[tauri::command] functions in src-tauri/src/commands/
 */
import { invoke } from "@tauri-apps/api/core";
import type {
  LearnerProfile,
  LearningTrack,
  LearningPath,
  ModuleProgress,
  SRCard,
  ReviewResult,
} from "@/types";
import type {
  AIProviderConfig,
  AssessKnowledgeRequest,
  AssessKnowledgeResponse,
  GeneratePathRequest,
  TutorMessage,
} from "@/types/ai";

// ── Learner Profile ──

export async function getOrCreateProfile(): Promise<LearnerProfile> {
  return invoke("get_or_create_profile");
}

export async function updateProfile(profile: Partial<LearnerProfile>): Promise<LearnerProfile> {
  return invoke("update_profile", { profile });
}

// ── Learning Tracks ──

export async function listTracks(): Promise<LearningTrack[]> {
  return invoke("list_tracks");
}

export async function createTrack(topic: string, domainModule: string, goal: string): Promise<LearningTrack> {
  return invoke("create_track", { topic, domainModule, goal });
}

export async function getTrack(trackId: string): Promise<LearningTrack> {
  return invoke("get_track", { trackId });
}

export async function updateTrackStatus(trackId: string, status: string): Promise<void> {
  return invoke("update_track_status", { trackId, status });
}

// ── Learning Paths ──

export async function getPath(trackId: string): Promise<LearningPath> {
  return invoke("get_path", { trackId });
}

export async function getModuleProgress(trackId: string): Promise<ModuleProgress[]> {
  return invoke("get_module_progress", { trackId });
}

export async function updateModuleProgress(progress: Partial<ModuleProgress>): Promise<void> {
  return invoke("update_module_progress", { progress });
}

// ── AI ──

export async function getAIConfig(): Promise<AIProviderConfig> {
  return invoke("get_ai_config");
}

export async function updateAIConfig(config: AIProviderConfig): Promise<void> {
  return invoke("update_ai_config", { config });
}

export async function assessKnowledge(req: AssessKnowledgeRequest): Promise<AssessKnowledgeResponse> {
  return invoke("assess_knowledge", { request: req });
}

export async function generateLearningPath(req: GeneratePathRequest): Promise<LearningPath> {
  return invoke("generate_learning_path", { request: req });
}

export async function sendTutorMessage(msg: TutorMessage): Promise<string> {
  return invoke("send_tutor_message", { message: msg });
}

// ── Spaced Repetition ──

export async function getDueCards(): Promise<SRCard[]> {
  return invoke("get_due_cards");
}

export async function submitReview(result: ReviewResult): Promise<SRCard> {
  return invoke("submit_review", { result });
}
