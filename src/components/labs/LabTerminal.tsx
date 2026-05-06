// Wave 0 stub — Phase 03.1 plan 03.1-06 wires xterm.js v5 to the
// `lab://stdout/<sessionId>` Tauri event stream and forwards onData /
// onResize through `lab_pty_write` and `lab_pty_resize` IPC commands.

export interface LabTerminalProps {
  sessionId: string;
  /** Bubble cols/rows up to the parent so PTY resize tracks the canvas. */
  onResize?: (cols: number, rows: number) => void;
}

export function LabTerminal(_props: LabTerminalProps) {
  return <div data-testid="lab-terminal-canvas">TODO: 03.1-06</div>;
}
