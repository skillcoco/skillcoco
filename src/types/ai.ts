// ═══════════════════════════════════════════
// AI Provider Types
// ═══════════════════════════════════════════

export type AIProviderType = "anthropic" | "openai" | "gemini" | "ollama" | "custom";

export interface AIProviderConfig {
  type: AIProviderType;
  apiKey?: string;
  model: string;
  baseUrl?: string;
  maxTokens: number;
  temperature: number;
}

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

export interface AssessmentTurn {
  role: string;
  content: string;
}

export interface AssessmentRequest {
  topic: string;
  domain: string;
  messages: AssessmentTurn[];
}

export interface AssessKnowledgeResponse {
  skillLevel: Record<string, number>;
  knowledgeGaps: string[];
  recommendedStartingPoint: string;
  overallLevel: string;
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
  moduleContext?: string;
  history?: AIMessage[];
}
