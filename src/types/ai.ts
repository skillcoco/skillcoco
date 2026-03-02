// ═══════════════════════════════════════════
// AI Provider Types
// ═══════════════════════════════════════════

export type AIProviderType = "claude" | "openai" | "ollama" | "custom";

export interface AIProviderConfig {
  type: AIProviderType;
  apiKey?: string;
  model: string;
  baseUrl?: string; // for custom/ollama
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

// ── AI Tutor Request/Response Types ──

export interface AssessKnowledgeRequest {
  topic: string;
  learnerResponses: string[];
}

export interface AssessKnowledgeResponse {
  skillLevel: Record<string, number>; // subtopic -> 0-1 score
  knowledgeGaps: string[];
  recommendedStartingPoint: string;
  overallLevel: string;
}

export interface GeneratePathRequest {
  topic: string;
  assessment: AssessKnowledgeResponse;
  goals: string[];
  preferences: {
    learningStyle: string;
    sessionDuration: number;
    depth: "overview" | "standard" | "deep";
  };
}

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
  score: number; // 0-100
  feedback: string;
  misconceptions: string[];
  hints: string[];
  isCorrect: boolean;
}

export interface TutorMessage {
  message: string;
  context: {
    trackId: string;
    moduleId?: string;
    learnerHistory: string;
  };
}
