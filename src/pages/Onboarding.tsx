import { useState, useRef, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import {
  Sparkles,
  ArrowRight,
  ArrowLeft,
  Loader2,
  Send,
  User,
  Bot,
  CheckCircle2,
  Target,
} from "lucide-react";
import { cn } from "@/lib/utils";
import {
  createTrack,
  assessKnowledge,
  generateLearningPath,
} from "@/lib/tauri-commands";
import type { AssessmentTurn } from "@/types/ai";

type OnboardingStep = "topic" | "goals" | "assessment" | "generating";

const domainModules = [
  { id: "programming", label: "Programming Language", examples: "Rust, Go, Python, TypeScript" },
  { id: "devops", label: "DevOps & Infrastructure", examples: "Kubernetes, Docker, Terraform, CI/CD" },
  { id: "cloud", label: "Cloud Platforms", examples: "AWS, GCP, Azure" },
  { id: "concepts", label: "Concepts & Theory", examples: "System Design, Algorithms, Networking" },
  { id: "data", label: "Data & AI/ML", examples: "ML Engineering, Data Pipelines, LLMs" },
];

interface ChatMessage {
  role: "user" | "assistant";
  content: string;
}

export function Onboarding() {
  const navigate = useNavigate();
  const [step, setStep] = useState<OnboardingStep>("topic");
  const [topic, setTopic] = useState("");
  const [selectedDomain, setSelectedDomain] = useState("");
  const [goal, setGoal] = useState("");

  // Assessment state
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>([]);
  const [userInput, setUserInput] = useState("");
  const [isAssessing, setIsAssessing] = useState(false);
  const [assessmentComplete, setAssessmentComplete] = useState(false);
  const [assessmentResult, setAssessmentResult] = useState<{
    level: string;
    gaps: string[];
    strengths: string[];
  } | null>(null);
  const chatEndRef = useRef<HTMLDivElement>(null);

  // Generation state
  const [generationPhase, setGenerationPhase] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [chatMessages]);

  // Start assessment when entering the assessment step
  useEffect(() => {
    if (step === "assessment" && chatMessages.length === 0) {
      startAssessment();
    }
  }, [step]);

  async function startAssessment() {
    setIsAssessing(true);
    try {
      const response = await assessKnowledge({
        topic,
        domain: selectedDomain,
        messages: [],
      });
      setChatMessages([{ role: "assistant", content: response }]);
    } catch (err) {
      setError(`Assessment failed: ${err}`);
    } finally {
      setIsAssessing(false);
    }
  }

  async function sendAssessmentMessage() {
    if (!userInput.trim() || isAssessing) return;

    const userMessage = userInput.trim();
    setUserInput("");
    const newMessages: ChatMessage[] = [
      ...chatMessages,
      { role: "user", content: userMessage },
    ];
    setChatMessages(newMessages);
    setIsAssessing(true);

    try {
      const turns: AssessmentTurn[] = newMessages.map((m) => ({
        role: m.role,
        content: m.content,
      }));

      const response = await assessKnowledge({
        topic,
        domain: selectedDomain,
        messages: turns,
      });

      setChatMessages([...newMessages, { role: "assistant", content: response }]);

      // Check if assessment is complete by looking for the JSON block
      const jsonMatch = response.match(/```json\s*(\{[\s\S]*?\})\s*```/);
      if (jsonMatch) {
        try {
          const parsed = JSON.parse(jsonMatch[1]);
          if (parsed.assessment_complete) {
            setAssessmentComplete(true);
            setAssessmentResult({
              level: parsed.level || "beginner",
              gaps: parsed.gaps || [],
              strengths: parsed.strengths || [],
            });
          }
        } catch {
          // Not valid JSON, assessment continues
        }
      }
    } catch (err) {
      setError(`Assessment failed: ${err}`);
    } finally {
      setIsAssessing(false);
    }
  }

  async function generatePath() {
    setStep("generating");
    setError(null);

    try {
      // Create the track first
      setGenerationPhase("Creating your learning track...");
      const track = await createTrack(topic, selectedDomain, goal);

      // Generate the learning path
      setGenerationPhase("AI is designing your personalized curriculum...");
      await generateLearningPath({
        trackId: track.id,
        topic,
        domain: selectedDomain,
        goal,
        assessmentLevel: assessmentResult?.level || "beginner",
        assessmentGaps: assessmentResult?.gaps || [],
        assessmentStrengths: assessmentResult?.strengths || [],
      });

      setGenerationPhase("Your learning path is ready!");
      setTimeout(() => navigate(`/track/${track.id}`), 1500);
    } catch (err) {
      setError(`Failed to generate path: ${err}`);
      setGenerationPhase("");
    }
  }

  function renderAssessmentMessage(content: string) {
    // Strip the JSON block from display
    return content.replace(/```json[\s\S]*?```/, "").trim();
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
            {step === "assessment" && "Let's assess your current knowledge"}
            {step === "generating" && "Creating your personalized path"}
          </p>
        </div>

        {/* Progress indicator */}
        <div className="flex items-center justify-center gap-2">
          {(["topic", "goals", "assessment", "generating"] as const).map((s, i) => (
            <div key={s} className="flex items-center gap-2">
              <div
                className={cn(
                  "flex h-8 w-8 items-center justify-center rounded-full text-xs font-medium",
                  step === s
                    ? "bg-primary text-primary-foreground"
                    : (["topic", "goals", "assessment", "generating"].indexOf(step) > i)
                      ? "bg-primary/20 text-primary"
                      : "bg-muted text-muted-foreground"
                )}
              >
                {i + 1}
              </div>
              {i < 3 && (
                <div
                  className={cn(
                    "h-0.5 w-8",
                    (["topic", "goals", "assessment", "generating"].indexOf(step) > i)
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
                Start Assessment
                <Target size={16} />
              </button>
            </div>
          </div>
        )}

        {/* Step: AI Assessment (conversational chat) */}
        {step === "assessment" && (
          <div className="glass rounded-xl overflow-hidden">
            {/* Chat messages */}
            <div className="h-96 overflow-y-auto p-4 space-y-4">
              {chatMessages.map((msg, i) => (
                <div
                  key={i}
                  className={cn(
                    "flex gap-3",
                    msg.role === "user" ? "flex-row-reverse" : ""
                  )}
                >
                  <div
                    className={cn(
                      "flex h-8 w-8 shrink-0 items-center justify-center rounded-full",
                      msg.role === "user"
                        ? "bg-primary/20 text-primary"
                        : "bg-muted text-muted-foreground"
                    )}
                  >
                    {msg.role === "user" ? <User size={14} /> : <Bot size={14} />}
                  </div>
                  <div
                    className={cn(
                      "rounded-lg px-4 py-3 text-sm max-w-[80%]",
                      msg.role === "user"
                        ? "bg-primary text-primary-foreground"
                        : "bg-muted/50 text-foreground"
                    )}
                  >
                    <p className="whitespace-pre-wrap">{renderAssessmentMessage(msg.content)}</p>
                  </div>
                </div>
              ))}
              {isAssessing && (
                <div className="flex gap-3">
                  <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-muted text-muted-foreground">
                    <Bot size={14} />
                  </div>
                  <div className="rounded-lg bg-muted/50 px-4 py-3">
                    <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
                  </div>
                </div>
              )}
              <div ref={chatEndRef} />
            </div>

            {/* Assessment complete banner */}
            {assessmentComplete && assessmentResult && (
              <div className="border-t border-border bg-primary/5 p-4">
                <div className="flex items-center gap-2 mb-2">
                  <CheckCircle2 className="h-4 w-4 text-primary" />
                  <span className="text-sm font-medium text-foreground">Assessment Complete</span>
                </div>
                <p className="text-xs text-muted-foreground mb-3">
                  Level: <span className="font-medium text-foreground capitalize">{assessmentResult.level}</span>
                  {assessmentResult.strengths.length > 0 && (
                    <> | Strengths: {assessmentResult.strengths.join(", ")}</>
                  )}
                </p>
                <button
                  onClick={generatePath}
                  className="flex w-full items-center justify-center gap-2 rounded-lg bg-primary px-4 py-3 text-sm font-medium text-primary-foreground hover:bg-primary/90"
                >
                  Generate My Learning Path
                  <Sparkles size={16} />
                </button>
              </div>
            )}

            {/* Chat input */}
            {!assessmentComplete && (
              <div className="border-t border-border p-3">
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={userInput}
                    onChange={(e) => setUserInput(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && !e.shiftKey && sendAssessmentMessage()}
                    placeholder="Type your answer..."
                    className="flex-1 rounded-lg border border-input bg-background px-4 py-2 text-sm focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                    disabled={isAssessing}
                    autoFocus
                  />
                  <button
                    onClick={sendAssessmentMessage}
                    disabled={!userInput.trim() || isAssessing}
                    className="flex items-center justify-center rounded-lg bg-primary px-3 py-2 text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
                  >
                    <Send size={16} />
                  </button>
                </div>
                <button
                  onClick={() => {
                    setAssessmentComplete(true);
                    setAssessmentResult({ level: "beginner", gaps: [], strengths: [] });
                  }}
                  className="mt-2 text-xs text-muted-foreground hover:text-foreground"
                >
                  Skip assessment (start as beginner)
                </button>
              </div>
            )}
          </div>
        )}

        {/* Step: Generating */}
        {step === "generating" && (
          <div className="glass rounded-xl p-8">
            <div className="flex flex-col items-center justify-center py-8 text-center">
              {error ? (
                <>
                  <div className="mb-4 text-destructive text-lg">Generation failed</div>
                  <button
                    onClick={() => {
                      setError(null);
                      setStep("assessment");
                    }}
                    className="text-sm text-primary hover:underline"
                  >
                    Go back and try again
                  </button>
                </>
              ) : (
                <>
                  <Loader2 className="mb-4 animate-spin text-primary" size={40} />
                  <h2 className="text-lg font-medium text-foreground mb-2">
                    {generationPhase || "Preparing..."}
                  </h2>
                  <p className="text-sm text-muted-foreground">
                    AI is creating a customized curriculum for {topic} based on your
                    background and goals
                  </p>
                </>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
