// Wave 0 failing tests for LabTerminal (LAB-02): xterm.js v5 binds to the
// `lab://stdout/<sessionId>` Tauri event and forwards onData/onResize
// through `lab_pty_write` / `lab_pty_resize` IPC.
//
// These tests use vi.mock to replace @xterm/xterm and @tauri-apps/api/event
// with recordable spies — there's no real PTY, no real Tauri bridge, and no
// jsdom canvas dependency.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "@testing-library/react";

const xtermState = vi.hoisted(() => ({
  open: vi.fn(),
  write: vi.fn(),
  dispose: vi.fn(),
  loadAddon: vi.fn(),
  onData: vi.fn(),
  onResize: vi.fn(),
}));

vi.mock("@xterm/xterm", () => ({
  Terminal: vi.fn().mockImplementation(() => xtermState),
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: vi.fn().mockImplementation(() => ({
    fit: vi.fn(),
    activate: vi.fn(),
    dispose: vi.fn(),
  })),
}));

vi.mock("@xterm/addon-web-links", () => ({
  WebLinksAddon: vi.fn().mockImplementation(() => ({
    activate: vi.fn(),
    dispose: vi.fn(),
  })),
}));

vi.mock("@xterm/addon-search", () => ({
  SearchAddon: vi.fn().mockImplementation(() => ({
    activate: vi.fn(),
    dispose: vi.fn(),
  })),
}));

vi.mock("@xterm/xterm/css/xterm.css", () => ({}));

const eventState = vi.hoisted(() => ({
  listenCalls: [] as Array<{ event: string; cb: (e: { payload: unknown }) => void }>,
  unlisten: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, cb: (e: { payload: unknown }) => void) => {
    eventState.listenCalls.push({ event, cb });
    return Promise.resolve(eventState.unlisten);
  }),
}));

const invokeFn = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeFn,
}));

import { LabTerminal } from "@/components/labs/LabTerminal";

describe("LabTerminal — Phase 03.1 Wave 0 (failing scaffolds)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    eventState.listenCalls = [];
    eventState.unlisten.mockReset();
    invokeFn.mockReset();
    invokeFn.mockResolvedValue(undefined);
  });

  it("lab_terminal_subscribes_to_stdout_event — listens on lab://stdout/<sessionId>", async () => {
    // FAILS until 03.1-06 wires xterm + tauri event listen.
    render(<LabTerminal sessionId="sess-1" />);
    // Allow microtask queue to flush so async useEffect runs.
    await Promise.resolve();
    await Promise.resolve();
    const stdoutListen = eventState.listenCalls.find(
      (c) => c.event === "lab://stdout/sess-1",
    );
    expect(stdoutListen).toBeDefined();
  });

  it("lab_terminal_writes_uint8array_on_payload — term.write called with Uint8Array", async () => {
    render(<LabTerminal sessionId="sess-1" />);
    await Promise.resolve();
    await Promise.resolve();
    const stdoutListen = eventState.listenCalls.find(
      (c) => c.event === "lab://stdout/sess-1",
    );
    if (!stdoutListen) {
      throw new Error("Expected listen on lab://stdout/sess-1 (FAILS in Wave 0)");
    }
    stdoutListen.cb({ payload: [104, 105] }); // "hi"
    expect(xtermState.write).toHaveBeenCalled();
    const arg = xtermState.write.mock.calls[0]?.[0];
    expect(arg).toBeInstanceOf(Uint8Array);
  });

  it("lab_terminal_invokes_pty_write_on_data — onData → lab_pty_write IPC", () => {
    render(<LabTerminal sessionId="sess-1" />);
    // onData was registered by the component during mount.
    expect(xtermState.onData).toHaveBeenCalled();
    const dataCb = xtermState.onData.mock.calls[0]?.[0] as ((s: string) => void) | undefined;
    if (!dataCb) {
      throw new Error("LabTerminal did not register an onData callback (FAILS in Wave 0)");
    }
    dataCb("hello");
    expect(invokeFn).toHaveBeenCalledWith(
      "lab_pty_write",
      expect.objectContaining({
        request: expect.objectContaining({ sessionId: "sess-1" }),
      }),
    );
  });

  it("lab_terminal_invokes_pty_resize_on_resize — onResize → lab_pty_resize IPC", () => {
    render(<LabTerminal sessionId="sess-1" />);
    expect(xtermState.onResize).toHaveBeenCalled();
    const resizeCb = xtermState.onResize.mock.calls[0]?.[0] as
      | ((d: { cols: number; rows: number }) => void)
      | undefined;
    if (!resizeCb) {
      throw new Error("LabTerminal did not register an onResize callback (FAILS in Wave 0)");
    }
    resizeCb({ cols: 120, rows: 30 });
    expect(invokeFn).toHaveBeenCalledWith(
      "lab_pty_resize",
      expect.objectContaining({
        request: expect.objectContaining({
          sessionId: "sess-1",
          cols: 120,
          rows: 30,
        }),
      }),
    );
  });

  it("lab_terminal_unlistens_on_unmount — disposes the stdout listener", async () => {
    const { unmount } = render(<LabTerminal sessionId="sess-1" />);
    await Promise.resolve();
    await Promise.resolve();
    unmount();
    // Allow the async unlisten cleanup to flush.
    await Promise.resolve();
    expect(eventState.unlisten).toHaveBeenCalled();
  });
});
