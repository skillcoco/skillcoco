// Wave 0 failing tests for LabBlock (LAB-10 split layout, LAB-02 lifecycle,
// LAB-03 host-shell-fallback warning). These FAIL today because LabBlock
// is a stub; plan 03.1-06 makes them green.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";

const labStoreState = vi.hoisted(() => ({
  openSession: vi.fn(),
  closeSession: vi.fn(),
  markStepComplete: vi.fn(),
  getProgress: vi.fn(),
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
  };
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
      <LabBlock block={block} learnerId="learner-1" trackId="trk-1" />
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
});
