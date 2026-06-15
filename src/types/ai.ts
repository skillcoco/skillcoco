// ═══════════════════════════════════════════
// AI Provider Types
// ═══════════════════════════════════════════

// AIProviderConfig removed in FIX-03 — auth flows through AuthState, not ai_config table.

export interface AIMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export interface AIConversation {
  id: string;
  trackId: string;
  moduleId: string | null;
  messages: AIMessage[];
  modelUsed: string;
  tokenCount: number;
  createdAt: string;
  updatedAt: string;
}

// ── Auth Types ──

export interface ProviderAuthStatus {
  provider: string;
  authenticated: boolean;
  method: string;
  displayName: string | null;
  model: string | null;
  isActive: boolean;
}

export interface LoginRequest {
  provider: string;
  method: "api-key" | "ollama";
  credential?: string;
  model?: string;
  baseUrl?: string;
}

// ── Assessment Types ──

export interface AssessmentRequest {
  topic: string;
  domain: string;
  level: "beginner" | "intermediate" | "advanced";
}

export interface AssessmentResult {
  assessment_complete: boolean;
  level: string;
  gaps: string[];
  strengths: string[];
}

// ── Path Generation ──

export interface GeneratePathRequest {
  trackId: string;
  topic: string;
  domain: string;
  goal: string;
  assessmentLevel: string;
  assessmentGaps: string[];
  assessmentStrengths: string[];
  /**
   * Phase 5 Q3 lock — when provided, the backend short-circuits AI generation
   * and builds the path directly from the named pack's modules + edges.
   * Free-text onboarding leaves this undefined for unchanged behavior.
   */
  packId?: string;
}

// ── Content Generation ──

export interface GenerateContentRequest {
  moduleId: string;
  trackId: string;
  moduleTitle: string;
  objectives: string[];
  learnerLevel: string;
  previousPerformance?: string;
}

// ── Exercise Types ──

export interface GenerateExerciseRequest {
  moduleId: string;
  difficulty: number;
  type: string;
  context: string;
}

export interface EvaluateResponseRequest {
  exercisePrompt: string;
  learnerResponse: string;
  rubric: string;
  expectedAnswer?: string;
}

export interface EvaluateResponseResult {
  score: number;
  feedback: string;
  misconceptions: string[];
  hints: string[];
  isCorrect: boolean;
}

// ── Tutor ──

export interface TutorMessage {
  content: string;
  /** When provided, backend fetches authoritative track + module context from DB. */
  moduleId?: string;
  trackId?: string;
  /** Display label only (backend ignores for grounding when moduleId is set). */
  moduleTitle?: string;
  /**
   * Block ID of the active section lesson (Phase 3 BLOCK-04).
   * Backend resolves this to section payload and uses it as primary tutor context.
   * Sticky on click (not scroll-derived) per CONTEXT.md locked decision.
   */
  currentSectionId?: string;
  /** @deprecated kept for one release — use moduleId instead. */
  moduleContext?: string;
  history?: AIMessage[];
}

// ── Provider Detection ──

export interface DetectedProvider {
  provider: string;
  source: string;
  imported: boolean;
}

// ── OAuth ──

export interface OAuthStartResult {
  started: boolean;
  provider: string;
}

export interface OAuthStatusResult {
  completed: boolean;
  provider: string;
  authenticated: boolean;
  /** Populated by FIX-01 when OAuth flow encounters an error. Absent on success. */
  error?: string;
}
