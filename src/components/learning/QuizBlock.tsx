import type { ModuleBlock, QuizAnswer } from "@/types/learning";

interface QuizBlockProps {
  block: ModuleBlock;
  onSubmit?: (answers: QuizAnswer[]) => void;
}

/**
 * One-question-per-card MCQ quiz with submit-at-end feedback.
 * Wave 4 (03-06 Task 1) implements card navigation, submit, review, and retake.
 * This placeholder allows Wave 0 test scaffolds to compile.
 */
export function QuizBlock({ block: _block, onSubmit: _onSubmit }: QuizBlockProps) {
  return <div data-testid="placeholder-quiz-block">Not implemented</div>;
}
