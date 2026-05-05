import { useEffect, useRef, useState, useId } from "react";

/**
 * Renders a fenced ```mermaid``` code block as an SVG diagram via mermaid.js.
 *
 * Loaded via dynamic import so the ~600 KB mermaid bundle stays out of the
 * initial app load — only learners viewing a lesson with a diagram pay the
 * cost. Each MermaidBlock manages its own render lifecycle: render on mount,
 * re-render on `code` change, render-error fallback to the raw source.
 */
interface MermaidBlockProps {
  code: string;
}

let mermaidPromise: Promise<typeof import("mermaid").default> | null = null;

function loadMermaid() {
  if (!mermaidPromise) {
    mermaidPromise = import("mermaid").then((m) => {
      const mermaid = m.default;
      mermaid.initialize({
        startOnLoad: false,
        theme: "dark",
        securityLevel: "loose",
        fontFamily: "inherit",
      });
      return mermaid;
    });
  }
  return mermaidPromise;
}

export function MermaidBlock({ code }: MermaidBlockProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const reactId = useId();
  const renderId = `mermaid-${reactId.replace(/[^a-z0-9]/gi, "")}`;
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setError(null);

    loadMermaid()
      .then(async (mermaid) => {
        try {
          const { svg } = await mermaid.render(renderId, code);
          if (!cancelled && containerRef.current) {
            containerRef.current.innerHTML = svg;
          }
        } catch (err) {
          if (!cancelled) {
            setError(String(err));
          }
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(`Failed to load mermaid: ${String(err)}`);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [code, renderId]);

  if (error) {
    return (
      <div className="my-4 rounded-md border border-amber-400/30 bg-amber-400/5 p-3 text-xs text-muted-foreground">
        <p className="mb-2 font-medium">Diagram couldn't render — showing source:</p>
        <pre className="overflow-x-auto whitespace-pre rounded bg-secondary/40 p-2 font-mono">
          {code}
        </pre>
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className="my-4 flex justify-center overflow-x-auto rounded-md border border-border bg-secondary/20 p-4"
      data-testid="mermaid-block"
      aria-label="Diagram"
    />
  );
}
