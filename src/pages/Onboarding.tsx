import { useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Sparkles,
  ArrowLeft,
  ArrowRight,
  Loader2,
} from "lucide-react";
import { cn } from "@/lib/utils";
import {
  createTrack,
  assessKnowledge,
  generateLearningPath,
} from "@/lib/tauri-commands";
import { PackPicker } from "@/components/onboarding/PackPicker";

// Phase 5 Plan 05 (Wave 4) — step 1 renamed from "topic" to "pack-picker"
// per D-08. The free-text fallback now lives inside <PackPicker /> as a
// collapsible. R5: the static domain-modules list moved into PackPicker
// (its canonical home now that the picker owns domain selection).
type OnboardingStep = "pack-picker" | "goals" | "assessment";

type LevelOption = "beginner" | "intermediate" | "advanced";

export function Onboarding() {
  const navigate = useNavigate();
  const [step, setStep] = useState<OnboardingStep>("pack-picker");
  const [topic, setTopic] = useState("");
  const [selectedDomain, setSelectedDomain] = useState("");
  const [goal, setGoal] = useState("");
  /**
   * `null` = free-text path (AI generates the curriculum). A non-null value
   * means the learner picked a Topic Pack; backend short-circuits AI and
   * snapshots the pack modules into learning_paths (D-11 immutability).
   */
  const [selectedPackId, setSelectedPackId] = useState<string | null>(null);

  // Assessment state
  const [selectedLevel, setSelectedLevel] = useState<LevelOption | null>(null);
  const [assessmentResult, setAssessmentResult] = useState<{
    level: string;
    gaps: string[];
    strengths: string[];
  } | null>(null);

  // Generation state
  const [isGenerating, setIsGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleLevelSelected() {
    if (!selectedLevel || isGenerating) return;
    setIsGenerating(true);
    setError(null);

    try {
      const response = await assessKnowledge({
        topic,
        domain: selectedDomain,
        level: selectedLevel,
      });
      const parsed = JSON.parse(response);
      const assessment = {
        level: parsed.level || selectedLevel,
        gaps: parsed.gaps || [],
        strengths: parsed.strengths || [],
      };
      setAssessmentResult(assessment);
      await generatePath(assessment);
    } catch (err) {
      setError(`Failed to create learning path: ${err}`);
      setIsGenerating(false);
    }
  }

  async function generatePath(overrideAssessment?: { level: string; gaps: string[]; strengths: string[] }) {
    setError(null);
    const assessment = overrideAssessment || assessmentResult;

    try {
      const track = await createTrack(topic, selectedDomain, goal);

      await generateLearningPath({
        trackId: track.id,
        topic,
        domain: selectedDomain,
        goal,
        assessmentLevel: assessment?.level || "beginner",
        assessmentGaps: assessment?.gaps || [],
        assessmentStrengths: assessment?.strengths || [],
        // Phase 5 Q3 — when learner picked a pack, packId routes the backend
        // to the short-circuit branch; otherwise undefined preserves the
        // existing AI-generated path (free-text fallback unchanged).
        packId: selectedPackId ?? undefined,
      });

      navigate(`/track/${track.id}`);
    } catch (err) {
      setError(`Failed to create learning path: ${err}`);
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-background p-6">
      <div className="w-full max-w-2xl space-y-8">
        {/* Header */}
        <div className="text-center">
          <Sparkles className="mx-auto mb-3 text-primary" size={32} />
          <h1 className="text-2xl font-bold text-foreground">Start a New Learning Track</h1>
          <p className="text-sm text-muted-foreground">
            {step === "pack-picker" && "Pick a topic pack or describe what you want to learn"}
            {step === "goals" && "Set your learning goals"}
            {step === "assessment" && "Rate your experience level"}
          </p>
        </div>

        {/* Progress indicator */}
        <div className="flex items-center justify-center gap-2">
          {(["pack-picker", "goals", "assessment"] as const).map((s, i) => (
            <div key={s} className="flex items-center gap-2">
              <div
                className={cn(
                  "flex h-8 w-8 items-center justify-center rounded-full text-xs font-medium",
                  step === s
                    ? "bg-primary text-primary-foreground"
                    : (["pack-picker", "goals", "assessment"].indexOf(step) > i)
                      ? "bg-primary/20 text-primary"
                      : "bg-muted text-muted-foreground"
                )}
              >
                {i + 1}
              </div>
              {i < 2 && (
                <div
                  className={cn(
                    "h-0.5 w-8",
                    (["pack-picker", "goals", "assessment"].indexOf(step) > i)
                      ? "bg-primary/40"
                      : "bg-muted"
                  )}
                />
              )}
            </div>
          ))}
        </div>

        {/* Error display */}
        {error && (
          <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-4 text-sm text-destructive">
            {error}
            <button
              onClick={() => setError(null)}
              className="ml-2 underline"
            >
              Dismiss
            </button>
          </div>
        )}

        {/* Step: Pack Picker (D-08 — replaces the free-text topic step) */}
        {step === "pack-picker" && (
          <PackPicker
            onPick={(packId, packTopic, domainModule) => {
              setTopic(packTopic);
              setSelectedDomain(domainModule);
              setSelectedPackId(packId);
              setStep("goals");
            }}
            onCustomTopic={(text, domain) => {
              setTopic(text);
              setSelectedDomain(domain);
              setSelectedPackId(null);
              setStep("goals");
            }}
          />
        )}

        {/* Step: Goals */}
        {step === "goals" && (
          <div className="glass rounded-xl p-6 space-y-6">
            <div>
              <label className="block text-sm font-medium text-foreground mb-2">
                What's your goal with {topic}?
              </label>
              <textarea
                value={goal}
                onChange={(e) => setGoal(e.target.value)}
                placeholder="e.g., Pass the CKA exam, Build production-grade clusters, Understand core concepts..."
                rows={3}
                className="w-full rounded-lg border border-input bg-background px-4 py-3 text-sm focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary resize-none"
                autoFocus
              />
            </div>
            <div className="flex gap-3">
              <button
                onClick={() => setStep("pack-picker")}
                className="flex items-center gap-2 rounded-lg border border-border px-4 py-3 text-sm font-medium hover:bg-accent"
              >
                <ArrowLeft size={16} />
                Back
              </button>
              <button
                onClick={() => setStep("assessment")}
                disabled={!goal}
                className="flex flex-1 items-center justify-center gap-2 rounded-lg bg-primary px-4 py-3 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
              >
                Continue
                <ArrowRight size={16} />
              </button>
            </div>
          </div>
        )}

        {/* Step: Level Selection */}
        {step === "assessment" && (
          <div className="glass rounded-xl p-6 space-y-6">
            <div>
              <label className="block text-sm font-medium text-foreground mb-4">
                How would you rate your current {topic} knowledge?
              </label>
              <div className="grid gap-3">
                {([
                  {
                    level: "beginner" as const,
                    title: "Beginner",
                    description: `New to ${topic} or just getting started with the basics`,
                  },
                  {
                    level: "intermediate" as const,
                    title: "Intermediate",
                    description: `Comfortable with ${topic} fundamentals, ready for deeper concepts`,
                  },
                  {
                    level: "advanced" as const,
                    title: "Advanced",
                    description: `Experienced with ${topic}, looking to fill gaps and master edge cases`,
                  },
                ]).map((card) => (
                  <button
                    key={card.level}
                    onClick={() => setSelectedLevel(card.level)}
                    className={cn(
                      "flex flex-col items-start rounded-lg border p-4 text-left transition-colors",
                      selectedLevel === card.level
                        ? "border-primary bg-primary/5"
                        : "border-border hover:border-primary/50"
                    )}
                  >
                    <span className="font-medium text-foreground">{card.title}</span>
                    <span className="text-xs text-muted-foreground mt-1">{card.description}</span>
                  </button>
                ))}
              </div>
            </div>
            <div className="flex gap-3">
              <button
                onClick={() => setStep("goals")}
                className="flex items-center gap-2 rounded-lg border border-border px-4 py-3 text-sm font-medium hover:bg-accent"
              >
                <ArrowLeft size={16} />
                Back
              </button>
              <button
                onClick={handleLevelSelected}
                disabled={!selectedLevel || isGenerating}
                className="flex flex-1 items-center justify-center gap-2 rounded-lg bg-primary px-4 py-3 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
              >
                {isGenerating ? (
                  <>
                    <Loader2 size={16} className="animate-spin" />
                    Building your {topic} curriculum...
                  </>
                ) : (
                  <>
                    Create Learning Path
                    <Sparkles size={16} />
                  </>
                )}
              </button>
            </div>
            {error && (
              <p className="text-xs text-destructive">{error}</p>
            )}
          </div>
        )}

      </div>
    </div>
  );
}
