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

export type BlockType = "section" | "text" | "callout" | "quiz" | "flash_cards";
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
