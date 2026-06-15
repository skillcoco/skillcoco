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

export async function deleteTrack(trackId: string): Promise<void> {
  return invoke("delete_track", { trackId });
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

// ── Phase 3 Block Commands (stubs — implemented in Wave 2/3) ──

export async function getModuleBlocks(moduleId: string): Promise<import("@/types/learning").ModuleBlock[]> {
  return invoke("get_module_blocks", { moduleId });
}

export async function generateModuleBlocks(
  req: import("@/types/learning").GenerateModuleBlocksRequest,
): Promise<import("@/types/learning").GenerateModuleBlocksResult> {
  return invoke("generate_module_blocks", { req });
}

export async function markLessonComplete(moduleId: string, blockId: string): Promise<void> {
  return invoke("mark_lesson_complete", { req: { moduleId, blockId } });
}

export async function getLessonCompletions(moduleId: string): Promise<string[]> {
  return invoke("get_lesson_completions", { moduleId });
}

export async function submitQuiz(
  req: import("@/types/learning").SubmitQuizRequest,
): Promise<import("@/types/learning").SubmitQuizResult> {
  return invoke("submit_quiz", { req });
}

export async function rateFlashCard(
  req: import("@/types/learning").RateFlashCardRequest,
): Promise<{ masteryLevel: number }> {
  return invoke("rate_flash_card", { req });
}

// ── Phase 3 Wave 2: Block regeneration commands (03-03) ──

export async function regenerateLesson(
  req: import("@/types/learning").RegenerateLessonRequest,
): Promise<import("@/types/learning").ModuleBlock> {
  return invoke("regenerate_lesson", { req });
}

export async function regenerateModule(
  req: import("@/types/learning").RegenerateModuleRequest,
): Promise<import("@/types/learning").GenerateModuleBlocksResult> {
  return invoke("regenerate_module", { req });
}

// ── Phase 03.1: Lab block IPC wrappers ──
//
// All 9 commands forward a `request` payload that matches the
// camelCase Rust IPC structs in `src-tauri/src/commands/labs/`. The
// argument key MUST be `request` to align with the Rust handler
// signatures (verified in 03.1-05 SUMMARY).

export async function labSessionOpen(
  request: import("@/types/learning").LabSessionOpenRequest,
): Promise<import("@/types/learning").LabSessionOpenResult> {
  return invoke("lab_session_open", { request });
}

export async function labSessionClose(
  request: import("@/types/learning").LabSessionCloseRequest,
): Promise<void> {
  return invoke("lab_session_close", { request });
}

export async function labPtyWrite(
  request: import("@/types/learning").LabPtyWriteRequest,
): Promise<void> {
  return invoke("lab_pty_write", { request });
}

export async function labPtyResize(
  request: import("@/types/learning").LabPtyResizeRequest,
): Promise<void> {
  return invoke("lab_pty_resize", { request });
}

export async function labCheckStep(
  request: import("@/types/learning").LabCheckStepRequest,
): Promise<import("@/types/learning").LabCheckStepResult> {
  return invoke("lab_check_step", { request });
}

export async function labShowHint(
  request: import("@/types/learning").LabShowHintRequest,
): Promise<import("@/types/learning").LabShowHintResult> {
  return invoke("lab_show_hint", { request });
}

export async function labReset(
  request: import("@/types/learning").LabResetRequest,
): Promise<import("@/types/learning").LabResetResult> {
  return invoke("lab_reset", { request });
}

export async function labGetProgress(
  request: import("@/types/learning").LabGetProgressRequest,
): Promise<import("@/types/learning").LabProgress> {
  return invoke("lab_get_progress", { request });
}

export async function labRuntimeDetect(
  request: import("@/types/learning").LabRuntimeDetectRequest = {},
): Promise<import("@/types/learning").LabRuntimeDetectResult> {
  return invoke("lab_runtime_detect", { request });
}

// ── Phase 4 Microlearning (Wave 0 — typed shells, Plan 03 wires Rust handlers) ──
//
// The Rust commands these wrap are defined as plain `pub async fn` in
// `src-tauri/src/commands/microlearning.rs`. Plan 03 will:
//   1. Add `#[tauri::command]` attributes
//   2. Register them in `tauri::generate_handler!`
//   3. Replace the `unimplemented!()` bodies with real logic
// Until then these wrappers will reject at the IPC layer (handler not found).

export interface DailyChallengePayload {
  blockId: string;
  blockType: string;
  moduleId: string;
  trackId: string;
  estMinutes: number;
  status: "pending" | "in_progress" | "done";
}

export interface GetDailyChallengeResult {
  challenge: DailyChallengePayload | null;
}

export interface CompleteDailyChallengeResult {
  newStreakDays: number;
  completedAt: string;
}

export interface IsDailyChallengeEnabledResult {
  enabled: boolean;
  globalStreakDays: number;
}

export async function getDailyChallenge(): Promise<GetDailyChallengeResult> {
  return invoke("get_daily_challenge", { request: {} });
}

export async function startDailyChallenge(challengeDate: string): Promise<void> {
  return invoke("start_daily_challenge", { request: { challengeDate } });
}

export async function completeDailyChallenge(
  challengeDate: string,
): Promise<CompleteDailyChallengeResult> {
  return invoke("complete_daily_challenge", { request: { challengeDate } });
}

export async function isDailyChallengeEnabled(): Promise<IsDailyChallengeEnabledResult> {
  return invoke("is_daily_challenge_enabled", { request: {} });
}
