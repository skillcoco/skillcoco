import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { Sparkles, ArrowRight, ArrowLeft, Loader2 } from "lucide-react";
import { cn } from "@/lib/utils";

type OnboardingStep = "topic" | "assessment" | "goals" | "generating";

const domainModules = [
  { id: "programming", label: "Programming Language", examples: "Rust, Go, Python, TypeScript" },
  { id: "devops", label: "DevOps & Infrastructure", examples: "Kubernetes, Docker, Terraform, CI/CD" },
  { id: "cloud", label: "Cloud Platforms", examples: "AWS, GCP, Azure" },
  { id: "concepts", label: "Concepts & Theory", examples: "System Design, Algorithms, Networking" },
  { id: "data", label: "Data & AI/ML", examples: "ML Engineering, Data Pipelines, LLMs" },
];

export function Onboarding() {
  const navigate = useNavigate();
  const [step, setStep] = useState<OnboardingStep>("topic");
  const [topic, setTopic] = useState("");
  const [selectedDomain, setSelectedDomain] = useState("");
  const [goal, setGoal] = useState("");

  return (
    <div className="flex min-h-screen items-center justify-center bg-background p-6">
      <div className="w-full max-w-lg space-y-8">
        {/* Header */}
        <div className="text-center">
          <Sparkles className="mx-auto mb-3 text-primary" size={32} />
          <h1 className="text-2xl font-bold text-foreground">Start a New Learning Track</h1>
          <p className="text-sm text-muted-foreground">
            Tell us what you want to learn and we'll create a personalized path
          </p>
        </div>

        {/* Step: Topic Selection */}
        {step === "topic" && (
          <div className="space-y-6">
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
          <div className="space-y-6">
            <div>
              <label className="block text-sm font-medium text-foreground mb-2">
                What's your goal with {topic}?
              </label>
              <textarea
                value={goal}
                onChange={(e) => setGoal(e.target.value)}
                placeholder="e.g., Pass the CKA exam, Build production-grade clusters, Understand core concepts..."
                rows={3}
                className="w-full rounded-lg border border-input bg-background px-4 py-3 text-sm focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
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
                Start Assessment
                <Sparkles size={16} />
              </button>
            </div>
          </div>
        )}

        {/* Step: AI Assessment (conversational) */}
        {step === "assessment" && (
          <div className="space-y-4">
            <div className="rounded-lg border border-border bg-card p-6">
              <p className="text-muted-foreground">
                AI conversational assessment will be implemented here. The tutor
                will ask questions to gauge your existing knowledge of {topic} and
                determine the right starting point.
              </p>
            </div>
            <button
              onClick={() => setStep("generating")}
              className="flex w-full items-center justify-center gap-2 rounded-lg bg-primary px-4 py-3 text-sm font-medium text-primary-foreground hover:bg-primary/90"
            >
              Generate My Learning Path
              <Sparkles size={16} />
            </button>
          </div>
        )}

        {/* Step: Generating */}
        {step === "generating" && (
          <div className="flex flex-col items-center justify-center py-12 text-center">
            <Loader2 className="mb-4 animate-spin text-primary" size={40} />
            <h2 className="text-lg font-medium text-foreground">
              Generating your personalized learning path...
            </h2>
            <p className="text-sm text-muted-foreground">
              AI is creating a customized curriculum for {topic} based on your
              background and goals
            </p>
            {/* In real implementation, this calls the AI and navigates to the track */}
            <button
              onClick={() => navigate("/")}
              className="mt-8 text-sm text-primary hover:underline"
            >
              Go to Dashboard (placeholder)
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
