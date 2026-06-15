/**
 * Topic Packs (Phase 5) IPC wrapper envelope-shape tests.
 *
 * Each wrapper in `src/lib/tauri-commands.ts` must call `invoke` with the
 * exact `{ request: T }` envelope the matching Rust handler in
 * `src-tauri/src/topic_packs/commands.rs` expects (Q9 lock).
 *
 * Tauri matches the top-level JS argument key to the Rust parameter name.
 * If a wrapper sends `{ req: ... }` but the Rust handler declares
 * `request: T`, the IPC silently fails. This file locks in the contract.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";

const invokeMock = vi.fn().mockResolvedValue(undefined);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import * as commands from "@/lib/tauri-commands";

describe("Phase 5 Topic Packs IPC envelope (Rust param name = `request`)", () => {
  beforeEach(() => {
    invokeMock.mockClear();
    invokeMock.mockResolvedValue(undefined);
  });

  it("listTopicPacks invokes `list_topic_packs` with NO payload", async () => {
    invokeMock.mockResolvedValueOnce([]);
    await commands.listTopicPacks();
    expect(invokeMock).toHaveBeenCalledWith("list_topic_packs");
    // No second arg — the Rust handler takes only `State<AppState>`.
    expect(invokeMock.mock.calls[0]).toHaveLength(1);
  });

  it("listTopicPacksAdmin invokes `list_topic_packs_admin` with NO payload", async () => {
    invokeMock.mockResolvedValueOnce([]);
    await commands.listTopicPacksAdmin();
    expect(invokeMock).toHaveBeenCalledWith("list_topic_packs_admin");
    expect(invokeMock.mock.calls[0]).toHaveLength(1);
  });

  it("setTopicPackEnabled sends { request: { packId, enabled } }", async () => {
    await commands.setTopicPackEnabled({ packId: "k8s", enabled: false });
    expect(invokeMock).toHaveBeenCalledWith("set_topic_pack_enabled", {
      request: { packId: "k8s", enabled: false },
    });
  });

  it("reloadSkills invokes `reload_skills` with NO payload", async () => {
    await commands.reloadSkills();
    expect(invokeMock).toHaveBeenCalledWith("reload_skills");
    expect(invokeMock.mock.calls[0]).toHaveLength(1);
  });

  it("getTopicPackModules sends { request: { packId } }", async () => {
    invokeMock.mockResolvedValueOnce({ modules: [], edges: [] });
    await commands.getTopicPackModules({ packId: "agentic-devops" });
    expect(invokeMock).toHaveBeenCalledWith("get_topic_pack_modules", {
      request: { packId: "agentic-devops" },
    });
  });
});
