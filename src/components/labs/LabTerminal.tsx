// Phase 03.1 plan 03.1-06 — xterm v5 + Tauri event bridge.
//
// Wires three Tauri events from the Rust runtime:
//   - lab://stdout/<sessionId>          → term.write(Uint8Array)
//   - lab://prompt-boundary/<sessionId> → useLabStore.markStepComplete(...)
//   - lab://session-ended/<sessionId>   → "session ended" notice (LAB-02)
// Forwards two Tauri IPC calls back to the runtime:
//   - term.onData    → lab_pty_write
//   - term.onResize  → lab_pty_resize
//
// Per RESEARCH § Determinism: tests vi.mock @xterm/xterm + @xterm/addon-*
// + @tauri-apps/api/event so the component can be exercised without a
// real DOM canvas, real PTY, or real Tauri bridge.
//
// Per CONTEXT.md "Claude's discretion" — PTY-died-recovery UX: we surface
// a passive "session ended" notice with a Restart button. The Restart
// button re-renders the LabBlock parent (state owned there) which
// triggers a fresh openSession. No automatic respawn.

import { useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { SearchAddon } from "@xterm/addon-search";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useTheme } from "@/hooks/useTheme";
import { useLabStore } from "@/stores/useLabStore";
import "@xterm/xterm/css/xterm.css";

export interface LabTerminalProps {
  sessionId: string;
  /** Bubble cols/rows up to the parent so PTY resize tracks the canvas. */
  onResize?: (cols: number, rows: number) => void;
}

interface PromptBoundaryPayload {
  command: string;
  output: string;
  exitCode: number | null;
  /** 0-based step index hint sent by the Rust prompt detector. */
  stepIndex?: number;
}

interface SessionEndedPayload {
  exitCode: number | null;
  reason: string;
}

const DARK_THEME = {
  background: "#0e1118",
  foreground: "#e6e9ef",
  cursor: "#f59e0b",
  selectionBackground: "rgba(245, 158, 11, 0.3)",
};

const LIGHT_THEME = {
  background: "#fafafa",
  foreground: "#1a1f2c",
  cursor: "#ea580c",
  selectionBackground: "rgba(234, 88, 12, 0.25)",
};

export function LabTerminal({ sessionId, onResize }: LabTerminalProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const { theme } = useTheme();
  const [sessionEnded, setSessionEnded] = useState<SessionEndedPayload | null>(
    null,
  );

  useEffect(() => {
    const container = containerRef.current;
    // The xterm Terminal API needs a DOM node; in test environments the
    // Terminal constructor is mocked so `open` is a no-op spy and the
    // null container path doesn't matter.
    const term = new Terminal({
      cursorBlink: true,
      scrollback: 5000,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, monospace",
      fontSize: 13,
      theme: theme === "dark" ? DARK_THEME : LIGHT_THEME,
    });
    const fit = new FitAddon();
    const webLinks = new WebLinksAddon();
    const search = new SearchAddon();
    term.loadAddon(fit);
    term.loadAddon(webLinks);
    term.loadAddon(search);

    if (container) {
      term.open(container);
      try {
        fit.fit();
      } catch {
        // jsdom doesn't implement layout; ignore.
      }
    }

    // ── Tauri events ──
    const unlisteners: Array<Promise<() => void>> = [];

    const stdoutP = listen<number[]>(
      `lab://stdout/${sessionId}`,
      (e) => {
        const payload = e.payload;
        if (Array.isArray(payload)) {
          term.write(new Uint8Array(payload));
        }
      },
    );
    unlisteners.push(stdoutP);

    const boundaryP = listen<PromptBoundaryPayload>(
      `lab://prompt-boundary/${sessionId}`,
      (e) => {
        const { command, output, exitCode, stepIndex } = e.payload;
        const idx = typeof stepIndex === "number" ? stepIndex : 0;
        // Fire-and-forget; the store handles its own promise lifecycle.
        void useLabStore
          .getState()
          .markStepComplete(sessionId, idx, command, output, exitCode);
      },
    );
    unlisteners.push(boundaryP);

    const endedP = listen<SessionEndedPayload>(
      `lab://session-ended/${sessionId}`,
      (e) => {
        setSessionEnded(e.payload);
      },
    );
    unlisteners.push(endedP);

    // ── PTY plumbing ──
    term.onData((data: string) => {
      const bytes = Array.from(new TextEncoder().encode(data));
      void invoke("lab_pty_write", {
        request: { sessionId, data: bytes },
      });
    });

    term.onResize(({ cols, rows }: { cols: number; rows: number }) => {
      void invoke("lab_pty_resize", {
        request: { sessionId, cols, rows },
      });
      onResize?.(cols, rows);
    });

    // Window resize → refit.
    const handleResize = () => {
      try {
        fit.fit();
      } catch {
        // jsdom or pre-mount.
      }
    };
    window.addEventListener("resize", handleResize);

    return () => {
      window.removeEventListener("resize", handleResize);
      for (const p of unlisteners) {
        p.then((u) => u()).catch(() => {
          // Listener never landed — nothing to clean up.
        });
      }
      try {
        term.dispose();
      } catch {
        // Already disposed.
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, theme]);

  return (
    <div className="flex h-full w-full flex-col">
      {sessionEnded ? (
        <div
          role="status"
          data-testid="lab-session-ended"
          className="flex items-center justify-between gap-3 border-b border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground"
        >
          <span>
            Session ended ({sessionEnded.reason}). Reopen the lab to start
            a fresh shell.
          </span>
        </div>
      ) : null}
      <div
        ref={containerRef}
        data-testid="lab-terminal-canvas"
        className="h-full w-full bg-background"
      />
    </div>
  );
}
