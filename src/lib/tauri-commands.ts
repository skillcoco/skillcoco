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
import type {
  TopicPack,
  SetTopicPackEnabledRequest,
  GetTopicPackModulesRequest,
  PackModulesResult,
} from "@/types/topic-packs";

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

/**
 * Phase 10 Plan 03 — IPC wrapper for set_track_browse_mode.
 * Persists the per-track browse mode to the backend (validated there against
 * the {linear, free} whitelist — T-10-01 mitigation from Plan 10-01).
 */
export async function setTrackBrowseMode(trackId: string, mode: "linear" | "free"): Promise<void> {
  return invoke("set_track_browse_mode", { trackId, mode });
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

// ── Topic Packs (Phase 5) ──
//
// All wrappers use the `{ request }` envelope per Q9 lock — the Rust
// handlers in `src-tauri/src/topic_packs/commands.rs` declare their
// payload parameter as `request: T`, so Tauri matches the top-level JS
// key `"request"` to deserialize. Wrappers WITHOUT arguments invoke with
// no second arg (matches the no-payload Rust signatures).

/// List only the packs the user has explicitly enabled. Wave 4 Onboarding
/// picker filters via this.
export async function listTopicPacks(): Promise<TopicPack[]> {
  return invoke("list_topic_packs");
}

/// List EVERY loaded pack (enabled + disabled + error sentinels). Wave 3
/// Settings UI uses this to surface every pack — including failures — to
/// the learner.
export async function listTopicPacksAdmin(): Promise<TopicPack[]> {
  return invoke("list_topic_packs_admin");
}

/// Toggle a single pack's `enabled` flag. Updates both the in-memory
/// registry and the SQLite row atomically (T-05-08: rejects unknown ids
/// before SQL touches the DB).
export async function setTopicPackEnabled(
  request: SetTopicPackEnabledRequest,
): Promise<void> {
  return invoke("set_topic_pack_enabled", { request });
}

/// Re-scan `~/.learnforge/skills/` — Q6 skills-only (bundled packs are
/// compile-time frozen). Wave 3 Settings "Reload" button calls this.
export async function reloadSkills(): Promise<void> {
  return invoke("reload_skills");
}

/// Fetch a single pack's modules + edges array — feeds Wave 4's
/// track-creation flow. Errors with "Unknown pack id: …" on unknown ids.
export async function getTopicPackModules(
  request: GetTopicPackModulesRequest,
): Promise<PackModulesResult> {
  return invoke("get_topic_pack_modules", { request });
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

// ── Phase 4 Microlearning IPC wrappers ──
//
// All four wrappers use the `{ request }` envelope per FIX-02 + Phase
// 03.1-06 precedent (Q9 lock). Request payloads are empty objects in v1:
// challenge_date is derived server-side via SQL `date('now')` (Pitfall 7)
// and learner_id is resolved server-side from learner_profiles (T-04-09).
// The request envelope still exists so the contract can grow without
// breaking the IPC signature.
//
// Types live in @/types/learning (DailyChallengePayload,
// GetDailyChallengeResult, CompleteDailyChallengeResult,
// IsDailyChallengeEnabledResult).

export async function getDailyChallenge(): Promise<
  import("@/types/learning").GetDailyChallengeResult
> {
  return invoke("get_daily_challenge", { request: {} });
}

export async function startDailyChallenge(): Promise<void> {
  return invoke("start_daily_challenge", { request: {} });
}

export async function completeDailyChallenge(): Promise<
  import("@/types/learning").CompleteDailyChallengeResult
> {
  return invoke("complete_daily_challenge", { request: {} });
}

export async function isDailyChallengeEnabled(): Promise<
  import("@/types/learning").IsDailyChallengeEnabledResult
> {
  return invoke("is_daily_challenge_enabled", { request: {} });
}

/// Persist the learner's Daily Challenge opt-out preference (D-13 / Wave 5).
/// Writes `learner_profiles.preferences_json.dailyChallengeEnabled = <bool>`.
/// The next Dashboard mount picks up the new value via
/// `is_daily_challenge_enabled`.
export async function setDailyChallengeEnabled(enabled: boolean): Promise<void> {
  return invoke("set_daily_challenge_enabled", { request: { enabled } });
}

// ── Phase 6 (Certification) — Plan 06-03 (Wave 2) IPC wrappers ──
//
// All wrappers follow the `{ request: T }` envelope per CONVENTIONS.md Q9.
// `exportCertificate` + `exportBadge` drive native save-as via
// `@tauri-apps/plugin-dialog::save` and write bytes via
// `@tauri-apps/plugin-fs::writeFile` (A7 lock — Tauri sandbox-enforced
// path; no path traversal possible per T-06-11).

import { save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import type {
  Achievement,
  ExportBadgeRequest,
  ExportCertificateRequest,
  GetTrackCertificationsRequest,
  TrackCertifications,
  VerifySignatureRequest,
  VerifySignatureResult,
} from "@/types/achievements";

/// List the current learner's earned achievements (badges + certificates).
/// Frontend useAchievementsStore.loadAchievements() calls this on mount.
export async function listAchievements(): Promise<Achievement[]> {
  return invoke("list_achievements_for_learner");
}

/// Per-track earned-levels + next-level snapshot. TrackView's
/// CertificationProgress component reads this.
export async function getTrackCertifications(
  request: GetTrackCertificationsRequest,
): Promise<TrackCertifications> {
  return invoke("get_track_certifications", { request });
}

/// Render the certificate PDF and prompt the user to save it via the
/// native dialog. Returns the saved path or `null` on cancel.
export async function exportCertificate(
  request: ExportCertificateRequest,
  suggestedFilename: string,
): Promise<string | null> {
  const bytes: number[] = await invoke("export_certificate", { request });
  const path = await save({
    defaultPath: suggestedFilename,
    filters: [{ name: "PDF Certificate", extensions: ["pdf"] }],
  });
  if (!path) return null;
  await writeFile(path, new Uint8Array(bytes));
  return path;
}

/// Render the PNG badge and prompt the user to save it via the native
/// dialog. Returns the saved path or `null` on cancel.
export async function exportBadge(
  request: ExportBadgeRequest,
  suggestedFilename: string,
): Promise<string | null> {
  const bytes: number[] = await invoke("export_badge", { request });
  const path = await save({
    defaultPath: suggestedFilename,
    filters: [{ name: "PNG Badge", extensions: ["png"] }],
  });
  if (!path) return null;
  await writeFile(path, new Uint8Array(bytes));
  return path;
}

/// Verify a pasted base64-encoded signed payload against either the local
/// public key (default) or a user-provided PEM override.
export async function verifySignature(
  request: VerifySignatureRequest,
): Promise<VerifySignatureResult> {
  return invoke("verify_signature", { request });
}

// ── Phase 6 (Certification) — Plan 06-06 (Wave 5) Settings Verify panel ──
//
// Both wrappers are pure shims around `signing::*` helpers. They power
// the Settings "Verify certificate" section: `getSigningPublicKey` feeds
// the "Show signing public key" clipboard export AND seeds the mount-time
// localFingerprint state (W4 fix — see SettingsVerifyCertSection.tsx);
// `fingerprintFromPublicPem` derives the 8-hex fingerprint from any PEM
// string (local or pasted override) without running a full verify pass.

/// Return the local install's Ed25519 signing public-key PEM
/// (`<app_data>/keys/cert_signing_public.pem`). Rejects with "Io ..." or
/// "not found"-style errors on the cold-start case where no certificate
/// has been issued yet (Phase 6 generates the keypair lazily on first
/// issuance per RESEARCH.md Pattern 2). Callers must absorb errors
/// silently — the Verify panel still works without a local key.
export async function getSigningPublicKey(): Promise<string> {
  return invoke("get_signing_public_key");
}

/// Derive the 8-hex SHA-256 fingerprint from a public-key PEM string.
/// Pure helper — no disk I/O. Enforces a 4KB cap on the input PEM
/// (T-06-22). The Settings Verify panel calls this on mount with the
/// local public PEM so the untrusted-signer warning fires on the FIRST
/// override paste — no prior verify pass required.
export async function fingerprintFromPublicPem(
  publicKeyPem: string,
): Promise<string> {
  return invoke("fingerprint_from_public_pem", {
    request: { publicKeyPem },
  });
}
