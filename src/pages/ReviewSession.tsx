import { useState } from "react";
import { Link } from "react-router-dom";
import { ArrowLeft, RotateCcw, Check, X } from "lucide-react";

export function ReviewSession() {
  const [started, setStarted] = useState(false);

  return (
    <div className="mx-auto max-w-2xl space-y-6">
      <div className="flex items-center gap-3">
        <Link to="/" className="rounded-md p-1.5 text-muted-foreground hover:bg-accent">
          <ArrowLeft size={18} />
        </Link>
        <h1 className="text-2xl font-bold text-foreground">Review Session</h1>
      </div>

      {!started ? (
        <div className="flex flex-col items-center justify-center rounded-lg border border-border py-16 text-center">
          <RotateCcw className="mb-4 text-primary" size={40} />
          <h2 className="text-lg font-medium text-foreground">Ready to review?</h2>
          <p className="mb-6 text-sm text-muted-foreground">
            Spaced repetition helps you retain what you learn
          </p>
          <button
            onClick={() => setStarted(true)}
            className="rounded-md bg-primary px-6 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
          >
            Start Review
          </button>
        </div>
      ) : (
        <div className="rounded-lg border border-border bg-card p-8 text-center">
          <p className="text-muted-foreground">
            Spaced repetition card UI will be implemented here (Phase 2).
          </p>
        </div>
      )}
    </div>
  );
}
