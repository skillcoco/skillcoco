// ═══════════════════════════════════════════
// Core Learning Domain Types
// ═══════════════════════════════════════════

export interface LearnerProfile {
  id: string;
  displayName: string;
  learningStyle: LearningStyle;
  experienceLevel: ExperienceLevel;
  /** JSON-encoded preferences string as sent by Rust backend (preferencesJson: String). */
  preferencesJson: string;
  createdAt: string;
  updatedAt: string;
  /**
   * Phase 08.2 — gamification points accumulator. Default 0. Awarded:
   * +10/quiz-pass, +50/module-completion, +100/milestone, +500/track-cert.
   * Optional in the type so older Rust shapes (pre-v010 migration)
   * still deserialize without crashing the frontend.
   */
  points?: number;
}

export type LearningStyle = "visual" | "textual" | "practical" | "theoretical" | "mixed";
export type ExperienceLevel = "beginner" | "intermediate" | "advanced" | "expert";

export interface LearnerPreferences {
  preferredSessionDuration: number; // minutes
  dailyGoalMinutes: number;
  notificationsEnabled: boolean;
  theme: "light" | "dark" | "system";
}

export interface LearningTrack {
  id: string;
  learnerId: string;
  topic: string;
  domainModule: DomainModule;
  status: TrackStatus;
  goal: string;
  currentModuleId: string | null;
  progressPercent: number;
  totalTimeSpent: number; // seconds
  createdAt: string;
  updatedAt: string;
  /** Days of consecutive daily activity on this track (FIX-04). 0 until first completion. */
  streakDays?: number;
  /** ISO datetime of last activity on this track (FIX-04). null until first completion. */
  lastActivityDate?: string | null;
  /**
   * Phase 10 Plan 03 — per-track browse mode.
   * "linear" (default): sequential unlock rules enforced.
   * "free": every module is openable; guidance (recommended-next) still shown.
   * Optional + default-linear so older cached rows without browse_mode are treated as linear.
   */
  browseMode?: "linear" | "free";
}

export type DomainModule = "programming" | "devops" | "concepts" | "data" | "cloud";
export type TrackStatus = "onboarding" | "active" | "paused" | "completed" | "archived";

export interface LearningPath {
  id: string;
  trackId: string;
  version: number;
  generatedByModel: string;
  /** JSON-encoded modules array as sent by Rust backend (get_path returns modulesJson). */
  modulesJson: string;
  /** JSON-encoded edges array as sent by Rust backend (get_path returns edgesJson). */
  edgesJson: string;
  /** @deprecated Pages should parse modulesJson instead. Kept for compatibility. */
  modules?: PathModule[];
  /** @deprecated Pages should parse edgesJson instead. Kept for compatibility. */
  edges?: PathEdge[];
  estimatedHours: number;
  createdAt: string;
  /**
   * Phase 14 Plan 04/05 (D-14) — signature-verification result for a
   * signed pack import, surfaced by the 14-04 import gate on the track
   * record (camelCase over IPC: `verified`/`issuerName`). `true` only when
   * a valid Ed25519 chain of trust (root -> issuer -> pack) was verified
   * at import time. Read-only display flag — NO cryptographic verification
   * runs in the browser; this field is never recomputed client-side.
   * Optional so pre-Phase-14 records (and unsigned imports) deserialize
   * without a verified badge.
   */
  verified?: boolean;
  /**
   * Phase 14 Plan 04/05 (D-14) — issuer name from the verified signing
   * cert, paired with `verified`. null when unverified or not present.
   */
  issuerName?: string | null;
}

export interface PathModule {
  id: string;
  title: string;
  description: string;
  type: ModuleType;
  difficulty: number; // 1-10
  estimatedMinutes: number;
  objectives: string[];
  prerequisites: string[]; // module IDs
}

export type ModuleType =
  | "lesson"        // Content + explanation
  | "exercise"      // Practice problems
  | "lab"           // Hands-on sandbox
  | "assessment"    // Knowledge check
  | "project";      // Mini-project combining concepts

export interface PathEdge {
  from: string; // module ID
  to: string;   // module ID
  type: "prerequisite" | "recommended" | "optional";
}

export interface ModuleProgress {
  id: string;
  moduleId: string;
  learnerId: string;
  status: ModuleStatus;
  score: number | null; // 0-100
  timeSpent: number; // seconds
  attempts: number;
  masteryLevel: number; // 0-1 (BKT probability)
  /**
   * Phase 03.1 LAB-08 — practical mastery dimension. Linear:
   * `completed_steps / total_steps` summed across the module's labs.
   * Defaults to 0 from the v006 migration. Selector helper:
   * `selectModulePracticalMastery(moduleId)` in `useLearningStore`.
   */
  practicalMastery: number;
  startedAt: string | null;
  completedAt: string | null;
}

export type ModuleStatus = "locked" | "available" | "in_progress" | "completed" | "skipped";

export interface ModuleContent {
  moduleId: string;
  sections: ContentSection[];
}

export interface ContentSection {
  id: string;
  type: "markdown" | "code" | "exercise" | "interactive";
  content: string;
  metadata?: Record<string, unknown>;
}

// ── Spaced Repetition ──

export interface SRCard {
  id: string;
  moduleId: string;
  concept: string;
  cardType: "active_recall" | "code_write" | "explain" | "apply";
  front: string;
  back: string;
  interval: number; // days
  easeFactor: number; // SM-2 ease factor (>= 1.3)
  repetitions: number;
  nextReview: string; // ISO date
  lastReview: string | null;
}

export interface ReviewResult {
  cardId: string;
  quality: 0 | 1 | 2 | 3 | 4 | 5; // SM-2 quality rating
  responseTime: number; // milliseconds
  response: string;
}

// ── Adaptation ──

export interface AdaptationEvent {
  id: string;
  trackId: string;
  eventType: AdaptationEventType;
  oldValue: string;
  newValue: string;
  reason: string;
  timestamp: string;
}

export type AdaptationEventType =
  | "difficulty_adjusted"
  | "module_inserted"
  | "module_skipped"
  | "module_reordered"
  | "path_regenerated"
  | "style_changed";

// ── Exercise Completion ──

export interface CompleteExercisesResult {
  masteryLevel: number;
  moduleCompleted: boolean;
  /** Module IDs that were unlocked by this completion (LOOP-02) */
  newlyUnlockedModuleIds: string[];
  /** SR cards created during this completion (LOOP-03) */
  cardsCreated: number;
}

// ── Phase 3: Block Taxonomy (BLOCK-01) ──

// NOTE (Phase 11 D-01): "video" is intentionally NOT a BlockType variant. Video content is
// rendered as a lesson-level adjunct panel (RelatedVideosPanel) mounted outside the block list,
// so the generation pipeline and block taxonomy remain untouched. See src/types/videos.ts.
export type BlockType = "section" | "text" | "callout" | "quiz" | "flash_cards" | "lab";
export type BlockStatus = "pending" | "generating" | "ready" | "failed";

/** Database row for module_blocks — crosses Tauri IPC boundary with camelCase keys. */
export interface ModuleBlock {
  id: string;
  moduleId: string;
  ordering: number;
  blockType: BlockType;
  status: BlockStatus;
  paramsJson: string;
  payloadJson: string;
  sourceAnchorsJson: string;
  metadataJson: string;
  retryCount: number;
  createdAt: string;
  updatedAt: string;
}

// ── Parsed payload types (parse payloadJson on the frontend) ──

export interface SectionPayload {
  markdown: string;
  wordCount?: number;
}

export interface TextPayload {
  markdown: string;
}

export interface CalloutPayload {
  variant: "info" | "warning" | "success" | "example" | "code" | "quote";
  title: string;
  body: string;
}

export interface QuizQuestion {
  id: string;
  stem: string;
  options: { id: string; text: string }[];
  correctOptionId: string; // option-id-based, shuffle-safe
  explanation: string;
}

export interface QuizPayload {
  questions: QuizQuestion[];
}

export interface FlashCard {
  id: string;
  front: string;
  back: string;
}

export interface FlashCardsPayload {
  cards: FlashCard[];
}

// ── Phase 3 IPC structs ──

export interface QuizAnswer {
  questionId: string;
  selectedOptionId: string;
}

export interface SubmitQuizRequest {
  moduleId: string;
  trackId: string;
  blockId: string;
  answers: QuizAnswer[];
}

export interface QuizQuestionReview {
  questionId: string;
  stem: string;
  learnerOptionId: string;
  correctOptionId: string;
  isCorrect: boolean;
  explanation: string;
}

export interface SubmitQuizResult {
  scorePercent: number;
  passed: boolean;
  masteryLevel: number;
  moduleCompleted: boolean;
  newlyUnlockedModuleIds: string[];
  cardsCreated: number;
  review: QuizQuestionReview[];
  /**
   * Phase 6 Wave 1 (A4 lock): achievements issued by this submission.
   * Empty array when no threshold was crossed. The frontend forwards this
   * to `useAchievementsStore.appendNewlyIssued` (sibling-slice — Phase 4
   * Pitfall 5 — see 06-04-PLAN.md). The Rust struct field is
   * `newly_issued_achievements: Vec<Achievement>` (camelCase via
   * #[serde(rename_all = "camelCase")]).
   */
  newlyIssuedAchievements: import("./achievements").Achievement[];
}

export interface GenerateModuleBlocksRequest {
  moduleId: string;
  trackId: string;
  moduleTitle: string;
  objectives: string[];
  learnerLevel: string;
}

export interface GenerateModuleBlocksResult {
  blocks: ModuleBlock[];
}

export interface RegenerateLessonRequest {
  blockId: string;
}

export interface RegenerateModuleRequest {
  moduleId: string;
  trackId: string;
}

export interface RateFlashCardRequest {
  blockId: string;
  cardId: string;
  moduleId: string;
  quality: number; // 1-5; >= 4 = "good/easy"
}

// ── Phase 03.1: Lab block taxonomy (LAB-01..LAB-10) ──
//
// Lab blocks embed an interactive PTY-backed terminal alongside step-by-step
// instructions. The TS surface here mirrors the Rust IPC contract from
// 03.1-RESEARCH.md § "Phase Requirements → Test Map" — every field crosses
// the Tauri boundary in camelCase per FIX-02 lesson.
//
// These types are consumed by the Wave 0 failing tests in this phase; the
// real implementations land in 03.1-06 (frontend) and 03.1-04..03.1-05
// (Rust IPC). Component stubs in src/components/labs/ render placeholders
// only — no real PTY, no real Tauri event, no real Zustand IPC yet.

/** Runtime selector exposed in Settings → Labs section (LAB-03). */
export type LabRuntimeChoice = "docker" | "hostShell" | "autoDetect";

/**
 * Discriminated union of step evaluation strategies (LAB-06).
 * Each `kind` selects which optional fields are meaningful — the Rust
 * evaluator validates field combinations per kind on parse.
 */
export interface StepCheck {
  kind: "command_regex" | "exit_code" | "file_state" | "ai_judge" | "command_absent";
  /** command_regex, command_absent: stdout/stderr regex pattern. */
  pattern?: string;
  /** command_regex, command_absent: when true, also match against stderr. */
  matchStderr?: boolean;
  /** exit_code: required exit status (defaults to 0 if unset). */
  expected?: number;
  /** file_state: path relative to /workspace. */
  path?: string;
  /** file_state: substrings the file body must contain. */
  contains?: string[];
  /** file_state: fixture path the file must equal byte-for-byte. */
  equalsFixture?: string;
  /** ai_judge: natural-language criterion for the LLM grader. */
  criteria?: string;
  /** ai_judge: confidence threshold (0-1). */
  threshold?: number;
}

export interface LabStep {
  id: string;
  title: string;
  prompt: string;
  check: StepCheck;
  /** Three-tier progressive hints: gentle nudge → partial → full solution. */
  hints: string[];
  /**
   * Phase 19.3 (D-03) — per-step validation grain override. Absent means
   * "no override" (matches the lab-level grain, or "step" if that's also
   * absent) — mirrors the Rust `Grain::Step` default / back-compat gate.
   */
  grain?: "step" | "milestone";
}

export interface LabSpec {
  slug: string;
  title: string;
  estimatedMinutes?: number;
  /** Hard requirement — when true, host-shell mode shows override notice. */
  requiresDocker: boolean;
  /** image XOR dockerfile — Rust-side spec parser enforces exclusivity. */
  image?: string;
  dockerfile?: string;
  /** Files this lab produces — used by surgical Reset (LAB-07). */
  creates: string[];
  steps: LabStep[];
  /**
   * Phase 19.3 (D-03) — whole-lab default validation grain. Absent means
   * "step" (back-compat: every pre-19.3 LAB.md has no `grain:` key and
   * renders identically to today — no Validate-milestone button).
   */
  grain?: "step" | "milestone";
}

/** Source markdown + generation prompt for regen (mirrors QuizPayload pattern). */
export interface LabBlockParams {
  source: string;
  generationPrompt: string;
}

/** Parsed lab spec stored in `payloadJson` after Rust-side gray_matter parse. */
export interface LabBlockPayload {
  spec: LabSpec;
}

/** Per-learner per-block lab progress row (LAB-08 migration v006). */
export interface LabProgress {
  blockId: string;
  currentStep: number;
  completedStepIds: string[];
  lastUpdated: string;
  /** Linear: completed_steps / total_steps across the module's labs. */
  practicalMastery: number;
}

/** Live PTY session handle returned by `lab_session_open`. */
export interface LabSession {
  sessionId: string;
  /** Host-shell fallback notice when Docker is not detected (LAB-03). */
  warning?: string;
  /** Resolved runtime — "docker" or "hostShell". */
  effectiveRuntime: "docker" | "hostShell";
  /**
   * Plan 03.1-09 GAP-05 — learner identity stashed on the session so
   * post-Pass progress refreshes don't need to re-thread it through every
   * `markStepComplete` call site. Optional for backward compat.
   */
  learnerId?: string;
}

// ── Phase 03.1 IPC request / response types (camelCase per FIX-02) ──
//
// These mirror the Rust IPC structs in `src-tauri/src/commands/labs/mod.rs`
// (all marked `#[serde(rename_all = "camelCase")]`). Field names and
// optionality must match the Rust side exactly so the typed wrappers in
// `src/lib/tauri-commands.ts` round-trip cleanly across the Tauri boundary.

export interface LabSessionOpenRequest {
  blockId: string;
  trackId: string;
  moduleId: string;
  learnerId: string;
}

export interface LabSessionOpenResult {
  sessionId: string;
  effectiveRuntime: "docker" | "hostShell";
  workspacePath: string;
  spec: LabSpec;
  progress: LabProgress;
  warning?: string;
}

export interface LabSessionCloseRequest {
  sessionId: string;
}

export interface LabPtyWriteRequest {
  sessionId: string;
  data: number[];
}

export interface LabPtyResizeRequest {
  sessionId: string;
  cols: number;
  rows: number;
}

export interface LabCheckStepRequest {
  sessionId: string;
  stepIndex: number;
  lastCommand: string;
  lastOutput: string;
  lastExitCode: number | null;
}

export interface LabCheckStepResult {
  stepIndex: number;
  passed: boolean;
  reason: string;
  checkKind: string;
  masteryDelta: number;
  /**
   * 19.3-REVIEW WR-03 — structural outcome from the Rust evaluator so the
   * UI never sniffs `reason` prose. "milestone_pending" is the D-04
   * prompt-boundary advisory on milestone-grain steps (not a real Fail).
   */
  outcome: "pass" | "fail" | "indeterminate" | "manual" | "milestone_pending";
}

/**
 * Phase 19.3 (D-04) — explicit milestone validation request. Carries only
 * the session id + step index; learner/module/block/workspace resolve
 * server-side from the session sidecar (mirrors `LabCheckStepRequest`'s
 * camelCase shape).
 */
export interface LabValidateMilestoneRequest {
  sessionId: string;
  stepIndex: number;
}

/** Phase 19.3 (D-04) — mirrors `LabCheckStepResult`'s shape. */
export interface LabValidateMilestoneResult {
  stepIndex: number;
  passed: boolean;
  reason: string;
  checkKind: string;
  masteryDelta: number;
  /**
   * WR-03 — structural outcome; never "milestone_pending" here (this
   * handler always evaluates), but typed identically for symmetry.
   */
  outcome: "pass" | "fail" | "indeterminate" | "manual" | "milestone_pending";
}

export interface LabShowHintRequest {
  sessionId: string;
  stepIndex: number;
  currentTier: number;
}

export interface LabShowHintResult {
  tier: number;
  text: string;
  finalTier: boolean;
}

export interface LabResetRequest {
  sessionId: string;
}

export interface LabResetResult {
  filesRemoved: string[];
  progressReset: boolean;
}

export interface LabGetProgressRequest {
  blockId: string;
  learnerId: string;
}

export interface LabRuntimeDetectRequest {
  /** Optional: defaults to `"autoDetect"` when omitted (Rust serde default). */
  setting?: LabRuntimeChoice;
}

export interface LabRuntimeDetectResult {
  dockerAvailable: boolean;
  dockerVersion: string | null;
  effectiveRuntime: "docker" | "hostShell";
  setting: LabRuntimeChoice;
}

// ── Phase 4 Microlearning types ──
//
// IPC result shapes for the four daily-challenge commands. All requests
// are empty (`{}`) — challenge_date and learner_id are derived server-side
// (Pitfall 7 + T-04-07 + T-04-09 mitigation), so the TS layer never models
// a request body. Wrappers in `@/lib/tauri-commands` invoke with the
// `{ request: {} }` envelope per Q9 + Phase 03.1-06 precedent.

export interface DailyChallengePayload {
  blockId: string;
  blockType: string;
  moduleId: string;
  trackId: string;
  estMinutes: number;
  /** Engagement-state machine — NOT the BlockStatus enum (R1). */
  status: "pending" | "in_progress" | "done";
}

export interface GetDailyChallengeResult {
  /** `null` when the learner has no candidate today (empty 0.3-0.7 BKT zone). */
  challenge: DailyChallengePayload | null;
}

export interface CompleteDailyChallengeResult {
  newStreakDays: number;
  /** SQLite UTC `YYYY-MM-DD HH:MM:SS`. */
  completedAt: string;
}

export interface IsDailyChallengeEnabledResult {
  /** True when D-12 auto-enable gate fires AND user has not opted out. */
  enabled: boolean;
  /** Returned alongside `enabled` so Dashboard mount needs only 2 IPCs (Pitfall 6). */
  globalStreakDays: number;
}
