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
} from "@/types";
import type {
  AssessmentRequest,
  GeneratePathRequest,
  GenerateContentRequest,
  TutorMessage,
  ProviderAuthStatus,
  LoginRequest,
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

// ── Auth ──

export async function getAuthStatus(): Promise<ProviderAuthStatus[]> {
  return invoke("get_auth_status");
}

export async function loginProvider(request: LoginRequest): Promise<ProviderAuthStatus> {
  return invoke("login_provider", { request });
}

export async function setActiveProvider(provider: string): Promise<void> {
  return invoke("set_active_provider", { provider });
}

export async function logoutProvider(provider: string): Promise<void> {
  return invoke("logout_provider", { provider });
}

// ── AI ──
// getAIConfig / updateAIConfig removed in FIX-03 — auth flows through AuthState commands.

export async function assessKnowledge(request: AssessmentRequest): Promise<string> {
  return invoke("assess_knowledge", { request });
}

export async function generateLearningPath(request: GeneratePathRequest): Promise<LearningPath> {
  return invoke("generate_learning_path", { request });
}

export async function sendTutorMessage(message: TutorMessage): Promise<string> {
  return invoke("send_tutor_message", { message });
}

// ── Module Content ──

export async function generateModuleContent(request: GenerateContentRequest): Promise<string> {
  return invoke("generate_module_content", { request });
}

// ── Exercises ──

export async function getExercises(moduleId: string): Promise<import("@/types/exercises").Exercise[]> {
  return invoke("get_exercises", { moduleId });
}

export async function generateExercise(req: import("@/types/ai").GenerateExerciseRequest): Promise<import("@/types/exercises").Exercise> {
  return invoke("generate_exercise", { request: req });
}

export async function evaluateResponse(req: import("@/types/ai").EvaluateResponseRequest): Promise<import("@/types/ai").EvaluateResponseResult> {
  return invoke("evaluate_response", { request: req });
}

// ── Module Completion ──

export async function completeModuleExercises(
  moduleId: string,
  trackId: string,
  scores: number[],
): Promise<import("@/types/learning").CompleteExercisesResult> {
  return invoke("complete_module_exercises", {
    request: { moduleId, trackId, scores },
  });
}

// ── OAuth ──

export async function startOAuthLogin(provider: string): Promise<import("@/types/ai").OAuthStartResult> {
  return invoke("start_oauth_login", { provider });
}

export async function checkOAuthStatus(provider: string): Promise<import("@/types/ai").OAuthStatusResult> {
  return invoke("check_oauth_status", { provider });
}

export async function saveSetupToken(token: string): Promise<import("@/types/ai").OAuthStartResult> {
  return invoke("save_setup_token", { token });
}

export async function detectSystemProviders(): Promise<import("@/types/ai").DetectedProvider[]> {
  return invoke("detect_system_providers");
}

// ── Spaced Repetition ──

export async function getDueCards(): Promise<SRCard[]> {
  return invoke("get_due_cards");
}

/// Result returned by `submit_review` — maps to Rust SubmitReviewResult (camelCase).
export interface SubmitReviewResult {
  newIntervalDays: number;
  nextReview: string; // ISO datetime
  easeFactor: number;
}

/// Submit a review for an SR card.
/// @param cardId  — the card ID to review
/// @param quality — SM-2 quality rating (1-5; 1=Again, 3=Hard, 4=Good, 5=Easy)
export async function submitReview(cardId: string, quality: number): Promise<SubmitReviewResult> {
  return invoke("submit_review", { result: { cardId, quality } });
}
