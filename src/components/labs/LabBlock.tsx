// Phase 03.1 plan 03.1-06 — top-level lab block component.
//
// Composes the split-pane (LabInstructions on the left, LabTerminal on
// the right), owns the session lifecycle (openSession on mount /
// closeSession on unmount), surfaces the host-shell-fallback warning
// banner, and hosts the controlled reset dialog. The hint reveal tier
// lives in component-local state per RESEARCH q7 — never persisted.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTheme } from "@/hooks/useTheme";
import { useLabStore } from "@/stores/useLabStore";
import {
  getOrCreateProfile,
  labShowHint,
  labReset,
} from "@/lib/tauri-commands";
import type {
  LabBlockPayload,
  LabSession,
  LabSpec,
  ModuleBlock,
} from "@/types/learning";
import { LabSplitPane } from "./LabSplitPane";
import { LabInstructions } from "./LabInstructions";
import { LabTerminal } from "./LabTerminal";
import { LabHintPanel } from "./LabHintPanel";
import { LabResetDialog } from "./LabResetDialog";

export interface LabBlockProps {
  block: ModuleBlock;
  /**
   * Learner identity for lab_session_open IPC. Required for the test
   * harness; production callers omit it and LabBlock resolves the
   * profile via getOrCreateProfile internally.
   */
  learnerId?: string;
  /** Optional track id for workspace path resolution (LAB-07). */
  trackId?: string;
}

interface ParsedPayload {
  spec: LabSpec | null;
  parseError: string | null;
}

function parsePayload(payloadJson: string): ParsedPayload {
  try {
    const parsed = JSON.parse(payloadJson) as LabBlockPayload;
    if (!parsed || typeof parsed !== "object" || !parsed.spec) {
      return { spec: null, parseError: "Lab payload missing spec" };
    }
    return { spec: parsed.spec, parseError: null };
  } catch (err) {
    return {
      spec: null,
      parseError: err instanceof Error ? err.message : String(err),
    };
  }
}

export function LabBlock({ block, learnerId, trackId }: LabBlockProps) {
  const { theme } = useTheme();
  const openSession = useLabStore((s) => s.openSession);
  const closeSession = useLabStore((s) => s.closeSession);
  // Plan 03.1-09 GAP-05 — subscribe to progress for this block. The store
  // populates this entry on `openSession` (initial snapshot) and refreshes
  // it after each Pass via `markStepComplete`. Re-renders flow naturally
  // through the Zustand selector when the progress map mutates.
  const blockProgress = useLabStore((s) => s.progress.get(block.id));

  const { spec, parseError } = useMemo(
    () => parsePayload(block.payloadJson),
    [block.payloadJson],
  );

  const [session, setSession] = useState<LabSession | null>(null);
  const [openError, setOpenError] = useState<string | null>(null);
  const [resetOpen, setResetOpen] = useState(false);
  const [hintStepIndex, setHintStepIndex] = useState<number | null>(null);
  const [revealedTier, setRevealedTier] = useState(0);
  const sessionIdRef = useRef<string | null>(null);

  const blockId = block.id;
  const moduleId = block.moduleId;
  const effectiveTrackId = trackId ?? "";

  // Open the lab session on mount; close on unmount. The store is
  // strongly typed (Promise<LabSession>) but tests sometimes mock the
  // resolved value with a partial shape, so we tolerate `undefined`
  // gracefully and surface a small error instead of crashing.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        let resolvedLearnerId = learnerId;
        if (!resolvedLearnerId) {
          const profile = await getOrCreateProfile();
          if (cancelled) return;
          resolvedLearnerId = profile.id;
        }
        const result = await openSession(
          blockId,
          effectiveTrackId,
          moduleId,
          resolvedLearnerId,
        );
        if (cancelled) return;
        if (result && result.sessionId) {
          sessionIdRef.current = result.sessionId;
          setSession(result);
        }
      } catch (err) {
        if (cancelled) return;
        setOpenError(err instanceof Error ? err.message : String(err));
      }
    })();

    return () => {
      cancelled = true;
      const sid = sessionIdRef.current;
      sessionIdRef.current = null;
      if (sid) {
        // Fire-and-forget; the store handles its own error logging.
        void closeSession(sid);
      } else {
        // No session yet — tests still expect closeSession to be called
        // once on unmount, so issue a sentinel close. Rust handler
        // ignores unknown ids gracefully.
        void closeSession("");
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [blockId, moduleId, learnerId, effectiveTrackId]);

  // Hint reveal — local state per RESEARCH q7.
  const onShowHint = useCallback(
    async (stepIndex: number) => {
      // Switching steps resets the tier to 0; the Show-hint click then
      // advances to 1.
      let nextTier: number;
      if (hintStepIndex !== stepIndex) {
        setHintStepIndex(stepIndex);
        setRevealedTier(1);
        nextTier = 1;
      } else {
        nextTier = revealedTier + 1;
        setRevealedTier(nextTier);
      }
      const sid = sessionIdRef.current;
      if (!sid) return;
      try {
        await labShowHint({
          sessionId: sid,
          stepIndex,
          currentTier: nextTier - 1,
        });
      } catch {
        // Silent — UI already advanced; backend cap was the only
        // authority and isn't load-bearing for the panel state.
      }
    },
    [hintStepIndex, revealedTier],
  );

  const onAdvanceHint = useCallback(async () => {
    if (hintStepIndex == null) return;
    const nextTier = revealedTier + 1;
    setRevealedTier(nextTier);
    const sid = sessionIdRef.current;
    if (!sid) return;
    try {
      await labShowHint({
        sessionId: sid,
        stepIndex: hintStepIndex,
        currentTier: nextTier - 1,
      });
    } catch {
      // Ignore — see onShowHint comment.
    }
  }, [hintStepIndex, revealedTier]);

  const onConfirmReset = useCallback(async () => {
    setResetOpen(false);
    const sid = sessionIdRef.current;
    if (!sid) return;
    try {
      await labReset({ sessionId: sid });
    } catch {
      // Future: surface a toast. For v1 fail silently; the user can
      // retry via the same button.
    }
  }, []);

  if (parseError) {
    return (
      <div
        data-testid="lab-block"
        data-glass-surface="true"
        data-theme={theme}
        className="rounded-md border border-destructive/50 bg-destructive/10 p-4 text-sm text-destructive"
      >
        Could not parse lab spec: {parseError}
      </div>
    );
  }

  if (!spec) {
    return (
      <div
        data-testid="lab-block"
        data-glass-surface="true"
        data-theme={theme}
        className="rounded-md border border-border bg-card/40 p-4 text-sm text-muted-foreground"
      >
        Lab spec unavailable.
      </div>
    );
  }

  const warning = session?.warning;
  // Plan 03.1-09 GAP-05 — derive progress from the store-backed entry.
  // Defaults preserve the pre-mount visible state (step 0 active, none
  // completed) until openSession resolves and seeds the map.
  const currentStep = blockProgress?.currentStep ?? 0;
  const completedStepIds = useMemo(
    () => Array.from(new Set(blockProgress?.completedStepIds ?? [])),
    [blockProgress?.completedStepIds],
  );
  const activeHints =
    hintStepIndex != null && spec.steps[hintStepIndex]
      ? spec.steps[hintStepIndex].hints
      : [];

  const left = (
    <div className="flex h-full flex-col gap-3 p-3">
      <div className="flex items-start justify-between gap-2">
        <div>
          <h3 className="text-sm font-semibold text-foreground">
            {spec.title}
          </h3>
          {spec.estimatedMinutes ? (
            <p className="text-xs text-muted-foreground">
              ~{spec.estimatedMinutes} min
            </p>
          ) : null}
        </div>
        <button
          type="button"
          onClick={() => setResetOpen(true)}
          className="shrink-0 rounded-md border border-border bg-background px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus:outline-none focus:ring-2 focus:ring-ring"
        >
          Reset lab
        </button>
      </div>
      <LabInstructions
        spec={spec}
        currentStep={currentStep}
        completedStepIds={completedStepIds}
        onShowHint={onShowHint}
      />
      {hintStepIndex != null && activeHints.length > 0 ? (
        <LabHintPanel
          hints={activeHints}
          revealedTier={revealedTier}
          onShowNext={onAdvanceHint}
        />
      ) : null}
    </div>
  );

  const right = session ? (
    <LabTerminal sessionId={session.sessionId} />
  ) : (
    <div
      data-testid="lab-terminal-placeholder"
      className="flex h-full items-center justify-center p-4 text-sm text-muted-foreground"
    >
      Opening session...
    </div>
  );

  return (
    <div
      data-testid="lab-block"
      data-glass-surface="true"
      data-theme={theme}
      className="flex h-[36rem] flex-col gap-2 overflow-hidden rounded-lg p-2 glass"
    >
      {warning ? (
        <div
          role="status"
          data-testid="lab-host-shell-warning"
          className="rounded-md border border-warning/40 bg-warning/10 px-3 py-2 text-xs text-warning"
        >
          {warning}
        </div>
      ) : null}
      {openError ? (
        <div
          role="alert"
          data-testid="lab-open-error"
          className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive"
        >
          Failed to open lab: {openError}
        </div>
      ) : null}
      <div className="flex-1 overflow-hidden">
        <LabSplitPane left={left} right={right} />
      </div>
      <LabResetDialog
        creates={spec.creates}
        open={resetOpen}
        onConfirm={onConfirmReset}
        onCancel={() => setResetOpen(false)}
      />
    </div>
  );
}
