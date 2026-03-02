// ═══════════════════════════════════════════
// Exercise Types
// ═══════════════════════════════════════════

export type ExerciseType =
  | "conceptual_qa"
  | "code_challenge"
  | "fill_in_blank"
  | "multiple_choice"
  | "architecture_design"
  | "scenario_debug"
  | "teach_back";

export interface Exercise {
  id: string;
  moduleId: string;
  type: ExerciseType;
  difficulty: number; // 1-10
  prompt: string;
  hints: string[];
  metadata: ExerciseMetadata;
}

export interface ExerciseMetadata {
  // For code_challenge
  language?: string;
  starterCode?: string;
  testCases?: TestCase[];

  // For fill_in_blank
  template?: string;
  blanks?: BlankDefinition[];

  // For multiple_choice
  options?: string[];
  correctIndices?: number[];

  // For architecture_design
  requirements?: string[];
  constraints?: string[];
}

export interface TestCase {
  input: string;
  expectedOutput: string;
  description: string;
  hidden: boolean;
}

export interface BlankDefinition {
  id: string;
  position: number;
  acceptedAnswers: string[];
  hint?: string;
}

export interface ExerciseAttempt {
  id: string;
  exerciseId: string;
  learnerId: string;
  response: string;
  score: number; // 0-100
  feedback: string;
  timeSpent: number; // seconds
  timestamp: string;
}
