// Wave 0 failing tests for LabBlock (LAB-10 split layout, LAB-02 lifecycle,
// LAB-03 host-shell-fallback warning). These FAIL today because LabBlock
// is a stub; plan 03.1-06 makes them green.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

const labStoreState = vi.hoisted(() => ({
  openSession: vi.fn(),
  closeSession: vi.fn(),
  markStepComplete: vi.fn(),
  validateMilestone: vi.fn(),
  getProgress: vi.fn(),
  // GAP-05 (Plan 03.1-09): progress map keyed by blockId. LabBlock reads
  // `useLabStore((s) => s.progress.get(blockId))` for currentStep +
  // completedStepIds; tests pre-seed this Map.
  progress: new Map<string, {
    blockId: string;
    currentStep: number;
    completedStepIds: string[];
    lastUpdated: string;
    practicalMastery: number;
  }>(),
}));

vi.mock("@/stores/useLabStore", () => ({
  useLabStore: vi.fn((selector?: (s: typeof labStoreState) => unknown) =>
    typeof selector === "function" ? selector(labStoreState) : labStoreState,
  ),
  __resetStore: vi.fn(),
}));

vi.mock("@/lib/tauri-commands", () => ({
  // Lab IPC commands are added by 03.1-05; this stub keeps imports resolvable.
  getLessonCompletions: vi.fn().mockResolvedValue([]),
  getOrCreateProfile: vi.fn().mockResolvedValue({ id: "learner-1" }),
  labShowHint: vi.fn().mockResolvedValue({ tier: 1, text: "", finalTier: false }),
  labReset: vi.fn().mockResolvedValue({ filesRemoved: [], progressReset: true }),
}));

vi.mock("@/hooks/useTheme", () => ({
  useTheme: vi.fn(() => ({ theme: "dark", setTheme: vi.fn(), toggleTheme: vi.fn() })),
}));

// LabTerminal is the real component now — it imports @xterm/xterm which
// touches jsdom-incompatible APIs (matchMedia). Mock the addon + xterm
// modules to keep the LabBlock test focused on the split-pane / lifecycle
// surface and out of the terminal canvas. The LabTerminal-specific test
// file owns the xterm + Tauri-event behaviour assertions.
vi.mock("@xterm/xterm", () => ({
  Terminal: vi.fn().mockImplementation(() => ({
    open: vi.fn(),
    write: vi.fn(),
    dispose: vi.fn(),
    loadAddon: vi.fn(),
    onData: vi.fn(),
    onResize: vi.fn(),
  })),
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: vi.fn().mockImplementation(() => ({ fit: vi.fn(), activate: vi.fn(), dispose: vi.fn() })),
}));

vi.mock("@xterm/addon-web-links", () => ({
  WebLinksAddon: vi.fn().mockImplementation(() => ({ activate: vi.fn(), dispose: vi.fn() })),
}));

vi.mock("@xterm/addon-search", () => ({
  SearchAddon: vi.fn().mockImplementation(() => ({ activate: vi.fn(), dispose: vi.fn() })),
}));

vi.mock("@xterm/xterm/css/xterm.css", () => ({}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

import { LabBlock } from "@/components/labs/LabBlock";
import type { ModuleBlock, LabBlockPayload } from "@/types/learning";

function makeLabBlock(payload: Partial<LabBlockPayload["spec"]> = {}): ModuleBlock {
  const spec = {
    slug: "pod-inspect",
    title: "Inspect a Pod",
    requiresDocker: true,
    image: "kindest/node:v1.30",
    creates: ["deploy.yaml"],
    steps: [
      {
        id: "s1",
        title: "List pods",
        prompt: "Run `kubectl get pods`",
        check: { kind: "command_regex" as const, pattern: "Running" },
        hints: ["a", "b", "c"],
      },
    ],
    ...payload,
  } as LabBlockPayload["spec"];
  return {
    id: "blk-lab-1",
    moduleId: "mod-1",
    ordering: 0,
    blockType: "lab",
    status: "ready",
    paramsJson: JSON.stringify({ source: "", generationPrompt: "" }),
    payloadJson: JSON.stringify({ spec }),
    sourceAnchorsJson: "[]",
    metadataJson: "{}",
    retryCount: 0,
    createdAt: "2026-05-05T00:00:00Z",
    updatedAt: "2026-05-05T00:00:00Z",
  };
}

function renderLabBlock(block = makeLabBlock()) {
  return render(
    <MemoryRouter>
      <LabBlock
        block={block}
        learnerId="learner-1"
        trackId="trk-1"
      />
    </MemoryRouter>,
  );
}

describe("LabBlock — Phase 03.1 Wave 0 (failing scaffolds)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    labStoreState.openSession.mockResolvedValue({
      sessionId: "sess-1",
      effectiveRuntime: "docker",
    });
    labStoreState.closeSession.mockResolvedValue(undefined);
    labStoreState.getProgress.mockResolvedValue({
      blockId: "blk-lab-1",
      currentStep: 0,
      completedStepIds: [],
      lastUpdated: "2026-05-06T00:00:00Z",
      practicalMastery: 0,
    });
    labStoreState.progress.clear();
  });

  it("lab_block_renders_60_40_split — split-pane container with role=separator divider", () => {
    // FAILS until 03.1-06 implements the split layout.
    renderLabBlock();
    expect(screen.getByTestId("lab-split-pane")).toBeInTheDocument();
    expect(screen.getByRole("separator")).toBeInTheDocument();
  });

  it("lab_block_lifecycle_open — invokes openSession on mount", () => {
    // FAILS until 03.1-06 wires useEffect mount → openSession.
    renderLabBlock();
    expect(labStoreState.openSession).toHaveBeenCalledTimes(1);
    expect(labStoreState.openSession).toHaveBeenCalledWith(
      "blk-lab-1",
      "trk-1",
      "mod-1",
      "learner-1",
    );
  });

  it("lab_block_lifecycle_close — invokes closeSession on unmount", () => {
    // FAILS until 03.1-06 wires useEffect cleanup → closeSession.
    const { unmount } = renderLabBlock();
    unmount();
    expect(labStoreState.closeSession).toHaveBeenCalledTimes(1);
  });

  it("lab_block_renders_host_shell_warning — surfaces 'Docker not detected' notice", async () => {
    // FAILS until 03.1-06 reads LabSession.warning and renders the notice.
    labStoreState.openSession.mockResolvedValue({
      sessionId: "sess-2",
      effectiveRuntime: "hostShell",
      warning: "Running on host shell — Docker not detected",
    });
    renderLabBlock();
    // Use findByText to wait for the post-mount openSession resolve.
    const notice = await screen.findByText(/host shell.*Docker not detected/i);
    expect(notice).toBeInTheDocument();
  });

  // ── GAP-05 (Plan 03.1-09): LabBlock reads progress from useLabStore ──────

  it("lab_block_renders_progress_from_store_state — currentStep + completedStepIds reflect store, not hardcoded zero", async () => {
    // Five-step lab; pre-seed the store with index=2, two steps already done.
    labStoreState.progress.set("blk-lab-1", {
      blockId: "blk-lab-1",
      currentStep: 2,
      completedStepIds: ["s1", "s2"],
      lastUpdated: "2026-05-06T00:00:00Z",
      practicalMastery: 0.4,
    });
    const fiveStepBlock = makeLabBlock({
      steps: [
        { id: "s1", title: "Step 1", prompt: "p1",
          check: { kind: "command_regex" as const, pattern: "x" }, hints: [] },
        { id: "s2", title: "Step 2", prompt: "p2",
          check: { kind: "command_regex" as const, pattern: "x" }, hints: [] },
        { id: "s3", title: "Step 3", prompt: "p3",
          check: { kind: "command_regex" as const, pattern: "x" }, hints: [] },
        { id: "s4", title: "Step 4", prompt: "p4",
          check: { kind: "command_regex" as const, pattern: "x" }, hints: [] },
        { id: "s5", title: "Step 5", prompt: "p5",
          check: { kind: "command_regex" as const, pattern: "x" }, hints: [] },
      ],
    });
    renderLabBlock(fiveStepBlock);

    // Step 2 (0-based index 2 = "Step 3") must be marked active; the first
    // two must be marked completed. FAILS today because LabBlock hardcodes
    // currentStep=0, completedStepIds=[] (lines 215-217).
    await waitFor(() => {
      expect(screen.getByTestId("lab-step-2")).toHaveAttribute("data-active", "true");
    });
    expect(screen.getByTestId("lab-step-0")).toHaveAttribute("data-completed", "true");
    expect(screen.getByTestId("lab-step-1")).toHaveAttribute("data-completed", "true");
    expect(screen.getByTestId("lab-step-3")).toHaveAttribute("data-completed", "false");
  });

  it("lab_block_refreshes_progress_after_pass — store mutation re-renders LabInstructions", async () => {
    // Mount with empty progress — currentStep=0 active.
    const fiveStepBlock = makeLabBlock({
      steps: [
        { id: "s1", title: "Step 1", prompt: "p1",
          check: { kind: "command_regex" as const, pattern: "x" }, hints: [] },
        { id: "s2", title: "Step 2", prompt: "p2",
          check: { kind: "command_regex" as const, pattern: "x" }, hints: [] },
        { id: "s3", title: "Step 3", prompt: "p3",
          check: { kind: "command_regex" as const, pattern: "x" }, hints: [] },
      ],
    });
    const { rerender } = renderLabBlock(fiveStepBlock);

    await waitFor(() => {
      expect(screen.getByTestId("lab-step-0")).toHaveAttribute("data-active", "true");
    });

    // Simulate the store mutation that follows a successful Pass: progress
    // entry for blockId now reports step 1 active, s1 completed.
    labStoreState.progress.set("blk-lab-1", {
      blockId: "blk-lab-1",
      currentStep: 1,
      completedStepIds: ["s1"],
      lastUpdated: "2026-05-06T00:01:00Z",
      practicalMastery: 0.33,
    });

    // Force re-render so the store selector re-runs against the new map.
    rerender(
      <MemoryRouter>
        <LabBlock block={fiveStepBlock} learnerId="learner-1" trackId="trk-1" />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("lab-step-1")).toHaveAttribute("data-active", "true");
    });
    expect(screen.getByTestId("lab-step-0")).toHaveAttribute("data-completed", "true");
  });

  // ── Phase 19.3-02 (D-04): conditional "Validate milestone" button ───────

  describe("Validate milestone button (D-04)", () => {
    beforeEach(() => {
      labStoreState.validateMilestone.mockResolvedValue({ outcome: "pass" });
    });

    it("shows the Validate milestone button when the current step's effective grain is milestone", async () => {
      const milestoneBlock = makeLabBlock({
        steps: [
          {
            id: "s1",
            title: "List pods",
            prompt: "Run `kubectl get pods`",
            check: { kind: "command_regex" as const, pattern: "Running" },
            hints: [],
            grain: "milestone",
          },
        ],
      });
      renderLabBlock(milestoneBlock);
      await waitFor(() => {
        expect(
          screen.getByRole("button", { name: /validate milestone/i }),
        ).toBeInTheDocument();
      });
    });

    it("shows the button for a LAB-LEVEL milestone spec in the Rust-serialized shape (spec.grain=milestone, steps[].grain='step') — CR-01", async () => {
      // serde serializes LabStep.grain with #[serde(default)] and NO
      // skip_serializing_if, so every step carries an explicit
      // "grain": "step". The backend rule (spec.rs::effective_step_grain)
      // is: step-level Milestone wins, OTHERWISE the lab-level grain
      // applies. The frontend must mirror that exactly — `step.grain ??
      // spec.grain` never falls through to the lab grain for this shape.
      const labLevelMilestoneBlock = makeLabBlock({
        grain: "milestone",
        steps: [
          {
            id: "s1",
            title: "List pods",
            prompt: "Run `kubectl get pods`",
            check: { kind: "command_regex" as const, pattern: "Running" },
            hints: [],
            grain: "step",
          },
        ],
      });
      renderLabBlock(labLevelMilestoneBlock);
      await waitFor(() => {
        expect(
          screen.getByRole("button", { name: /validate milestone/i }),
        ).toBeInTheDocument();
      });
    });

    it("renders NO Validate milestone button for a grain-absent (step) spec — back-compat", async () => {
      renderLabBlock(); // default makeLabBlock() has no `grain` anywhere
      await waitFor(() => {
        expect(screen.getByTestId("lab-block")).toBeInTheDocument();
      });
      expect(
        screen.queryByRole("button", { name: /validate milestone/i }),
      ).not.toBeInTheDocument();
    });

    it("clicking Validate milestone calls the store action with (sessionId, currentStep)", async () => {
      const milestoneBlock = makeLabBlock({
        steps: [
          {
            id: "s1",
            title: "List pods",
            prompt: "Run `kubectl get pods`",
            check: { kind: "command_regex" as const, pattern: "Running" },
            hints: [],
            grain: "milestone",
          },
        ],
      });
      const { default: userEvent } = await import("@testing-library/user-event");
      const user = userEvent.setup();
      renderLabBlock(milestoneBlock);

      const button = await screen.findByRole("button", {
        name: /validate milestone/i,
      });
      await user.click(button);

      await waitFor(() => {
        expect(labStoreState.validateMilestone).toHaveBeenCalledWith(
          "sess-1",
          0,
        );
      });
    });

    it("hides the button when currentStep is out of range (all steps done) — WR-04", async () => {
      // After the final milestone step passes, currentStep === steps.length;
      // the button must NOT render (clicking would invoke
      // lab_validate_milestone with an out-of-range index → Err).
      const milestoneBlock = makeLabBlock({
        grain: "milestone",
        steps: [
          {
            id: "s1",
            title: "List pods",
            prompt: "Run `kubectl get pods`",
            check: { kind: "command_regex" as const, pattern: "Running" },
            hints: [],
            grain: "step",
          },
        ],
      });
      labStoreState.progress.set("blk-lab-1", {
        blockId: "blk-lab-1",
        currentStep: 1, // === steps.length — lab complete
        completedStepIds: ["s1"],
        lastUpdated: "2026-05-06T00:00:00Z",
        practicalMastery: 1.0,
      });
      renderLabBlock(milestoneBlock);
      await waitFor(() => {
        expect(screen.getByTestId("lab-block")).toBeInTheDocument();
      });
      expect(
        screen.queryByRole("button", { name: /validate milestone/i }),
      ).not.toBeInTheDocument();
    });

    it("catches validateMilestone rejection — no unhandled promise rejection — WR-04", async () => {
      labStoreState.validateMilestone.mockRejectedValue(
        new Error("step_index 0 out of range"),
      );
      const milestoneBlock = makeLabBlock({
        steps: [
          {
            id: "s1",
            title: "List pods",
            prompt: "Run `kubectl get pods`",
            check: { kind: "command_regex" as const, pattern: "Running" },
            hints: [],
            grain: "milestone",
          },
        ],
      });
      const { default: userEvent } = await import("@testing-library/user-event");
      const user = userEvent.setup();
      renderLabBlock(milestoneBlock);

      const button = await screen.findByRole("button", {
        name: /validate milestone/i,
      });
      await user.click(button);

      await waitFor(() => {
        expect(labStoreState.validateMilestone).toHaveBeenCalledWith(
          "sess-1",
          0,
        );
      });
      // The rejection must be caught inside the handler; vitest fails the
      // test on an unhandled rejection, so reaching this point (and the
      // button re-enabling) is the assertion.
      await waitFor(() => {
        expect(
          screen.getByRole("button", { name: /validate milestone/i }),
        ).toBeEnabled();
      });
    });

    it("disables the button while validation is in flight (double-click guard, CR-02 defense-in-depth) — WR-04", async () => {
      let resolveValidation!: (v: { outcome: string }) => void;
      labStoreState.validateMilestone.mockImplementation(
        () =>
          new Promise<{ outcome: string }>((resolve) => {
            resolveValidation = resolve;
          }),
      );
      const milestoneBlock = makeLabBlock({
        steps: [
          {
            id: "s1",
            title: "List pods",
            prompt: "Run `kubectl get pods`",
            check: { kind: "command_regex" as const, pattern: "Running" },
            hints: [],
            grain: "milestone",
          },
        ],
      });
      const { default: userEvent } = await import("@testing-library/user-event");
      const user = userEvent.setup();
      renderLabBlock(milestoneBlock);

      const button = await screen.findByRole("button", {
        name: /validate milestone/i,
      });
      await user.click(button);

      await waitFor(() => {
        expect(
          screen.getByRole("button", { name: /validate milestone/i }),
        ).toBeDisabled();
      });

      resolveValidation({ outcome: "pass" });
      await waitFor(() => {
        expect(
          screen.getByRole("button", { name: /validate milestone/i }),
        ).toBeEnabled();
      });
      expect(labStoreState.validateMilestone).toHaveBeenCalledTimes(1);
    });
  });
});
