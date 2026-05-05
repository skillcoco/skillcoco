import { Component, type ReactNode, type ErrorInfo } from "react";
import { AlertTriangle, RotateCcw } from "lucide-react";

interface ErrorBoundaryProps {
  children: ReactNode;
  /** Optional label so different boundaries can identify themselves in logs. */
  scope?: string;
}

interface ErrorBoundaryState {
  error: Error | null;
}

/**
 * Top-level error boundary. Without this, an uncaught render error in any
 * deeply-nested component blanks the entire app screen — which is exactly
 * what the user hit during the acceptance walkthrough on 2026-05-05.
 *
 * On error: displays a friendly message + a Reset button that clears the
 * error state so the user doesn't have to reload the whole desktop app.
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // Log loud + structured so the Tauri devtools console makes the cause
    // obvious instead of just showing a blank screen.
    console.error(`[ErrorBoundary${this.props.scope ? `:${this.props.scope}` : ""}]`, error);
    console.error("Component stack:", info.componentStack);
  }

  reset = () => {
    this.setState({ error: null });
  };

  render() {
    if (this.state.error) {
      return (
        <div className="flex min-h-screen items-center justify-center p-6">
          <div className="glass max-w-lg space-y-4 rounded-xl border border-destructive/30 p-6">
            <div className="flex items-center gap-3">
              <AlertTriangle size={28} className="text-destructive" />
              <h2 className="text-xl font-semibold text-foreground">Something went wrong</h2>
            </div>
            <p className="text-sm text-muted-foreground">
              An unexpected error stopped this view from rendering. The error
              has been logged to the console. Try resetting; if it keeps
              happening, restart the app and report what you were doing.
            </p>
            <details className="rounded-md border border-border bg-secondary/30 p-3 text-xs">
              <summary className="cursor-pointer font-medium text-foreground">
                Error details
              </summary>
              <pre className="mt-2 whitespace-pre-wrap break-all text-muted-foreground">
                {this.state.error.message}
                {this.state.error.stack ? `\n\n${this.state.error.stack}` : ""}
              </pre>
            </details>
            <button
              onClick={this.reset}
              className="flex items-center gap-2 rounded-lg bg-primary px-4 py-2.5 text-sm font-medium text-primary-foreground hover:bg-primary/90"
            >
              <RotateCcw size={16} />
              <span>Reset and continue</span>
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
