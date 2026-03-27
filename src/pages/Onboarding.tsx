import { useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Sparkles,
  ArrowRight,
  ArrowLeft,
} from "lucide-react";
import { cn } from "@/lib/utils";
import {
  createTrack,
  assessKnowledge,
  generateLearningPath,
} from "@/lib/tauri-commands";

type OnboardingStep = "topic" | "goals" | "assessment";

const domainModules = [
  { id: "programming", label: "Programming Language", examples: "Rust, Go, Python, TypeScript" },
  { id: "devops", label: "DevOps & Infrastructure", examples: "Kubernetes, Docker, Terraform, CI/CD" },
  { id: "cloud", label: "Cloud Platforms", examples: "AWS, GCP, Azure" },
  { id: "concepts", label: "Concepts & Theory", examples: "System Design, Algorithms, Networking" },
  { id: "data", label: "Data & AI/ML", examples: "ML Engineering, Data Pipelines, LLMs" },
];

type LevelOption = "beginner" | "intermediate" | "advanced";

export function Onboarding() {
  const navigate = useNavigate();
  const [step, setStep] = useState<OnboardingStep>("topic");
  const [topic, setTopic] = useState("");
  const [selectedDomain, setSelectedDomain] = useState("");
  const [goal, setGoal] = useState("");

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
            {step === "topic" && "Tell us what you want to learn"}
            {step === "goals" && "Set your learning goals"}
            {step === "assessment" && "Rate your experience level"}
          </p>
        </div>

        {/* Progress indicator */}
        <div className="flex items-center justify-center gap-2">
          {(["topic", "goals", "assessment"] as const).map((s, i) => (
            <div key={s} className="flex items-center gap-2">
              <div
                className={cn(
                  "flex h-8 w-8 items-center justify-center rounded-full text-xs font-medium",
                  step === s
                    ? "bg-primary text-primary-foreground"
                    : (["topic", "goals", "assessment"].indexOf(step) > i)
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
                    (["topic", "goals", "assessment"].indexOf(step) > i)
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

        {/* Step: Topic Selection */}
        {step === "topic" && (
          <div className="glass rounded-xl p-6 space-y-6">
            <div>
              <label className="block text-sm font-medium text-foreground mb-2">
                What do you want to learn?
              </label>
              <input
                type="text"
                value={topic}
                onChange={(e) => setTopic(e.target.value)}
                placeholder="e.g., Kubernetes, Rust programming, System Design..."
                className="w-full rounded-lg border border-input bg-background px-4 py-3 text-sm focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                autoFocus
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-foreground mb-2">
                Domain
              </label>
              <div className="grid gap-2">
                {domainModules.map((dm) => (
                  <button
                    key={dm.id}
                    onClick={() => setSelectedDomain(dm.id)}
                    className={cn(
                      "flex flex-col items-start rounded-lg border p-3 text-left transition-colors",
                      selectedDomain === dm.id
                        ? "border-primary bg-primary/5"
                        : "border-border hover:border-primary/50"
                    )}
                  >
                    <span className="font-medium text-foreground">{dm.label}</span>
                    <span className="text-xs text-muted-foreground">{dm.examples}</span>
                  </button>
                ))}
              </div>
            </div>
            <button
              onClick={() => setStep("goals")}
              disabled={!topic || !selectedDomain}
              className="flex w-full items-center justify-center gap-2 rounded-lg bg-primary px-4 py-3 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              Continue
              <ArrowRight size={16} />
            </button>
          </div>
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
                onClick={() => setStep("topic")}
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
                {isGenerating ? "Creating..." : "Generate My Learning Path"}
                <Sparkles size={16} />
              </button>
            </div>
          </div>
        )}

      </div>
    </div>
  );
}
