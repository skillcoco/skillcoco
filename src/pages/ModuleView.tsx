import { useParams, Link } from "react-router-dom";
import { ArrowLeft, MessageCircle } from "lucide-react";

export function ModuleView() {
  const { trackId, moduleId } = useParams();

  return (
    <div className="mx-auto max-w-4xl space-y-6">
      <div className="flex items-center gap-3">
        <Link
          to={`/track/${trackId}`}
          className="rounded-md p-1.5 text-muted-foreground hover:bg-accent"
        >
          <ArrowLeft size={18} />
        </Link>
        <h1 className="text-2xl font-bold text-foreground">Module Content</h1>
      </div>

      {/* Module content will be rendered here - markdown, code blocks, exercises */}
      <div className="rounded-lg border border-border bg-card p-6">
        <p className="text-muted-foreground">
          Module content rendering will be implemented here. This will display
          markdown lessons, interactive code blocks, exercises, and connect to
          the AI tutor.
        </p>
        <p className="mt-2 text-sm text-muted-foreground">
          Module ID: {moduleId}
        </p>
      </div>

      {/* AI Tutor Toggle */}
      <button className="fixed bottom-16 right-6 flex h-12 w-12 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-lg hover:bg-primary/90">
        <MessageCircle size={20} />
      </button>
    </div>
  );
}
