/**
 * Phase 6 (Certification) IPC wrapper envelope-shape tests.
 *
 * Each wrapper in `src/lib/tauri-commands.ts` must call `invoke` with the
 * exact `{ request: T }` envelope the matching Rust handler in
 * `src-tauri/src/commands/achievements.rs` expects (Q9 lock). Tauri
 * matches the top-level JS argument key to the Rust parameter name. If a
 * wrapper sends `{ req: ... }` but the Rust handler declares `request: T`,
 * the IPC silently fails. This file locks in the contract.
 *
 * `exportCertificate` + `exportBadge` also go through the native save
 * dialog plugin; we mock those plugins here and confirm the file-write
 * path is exercised when the dialog returns a path (T-06-11: writeFile
 * is sandbox-enforced — we only assert that the wrapper forwards bytes,
 * not that the OS sandbox is actually applied).
 */
import { describe, it, expect, vi, beforeEach } from "vitest";

const invokeMock = vi.fn().mockResolvedValue(undefined);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

const saveMock = vi.fn();
vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: (...args: unknown[]) => saveMock(...args),
}));

const writeFileMock = vi.fn().mockResolvedValue(undefined);
vi.mock("@tauri-apps/plugin-fs", () => ({
  writeFile: (...args: unknown[]) => writeFileMock(...args),
}));

import * as commands from "@/lib/tauri-commands";

describe("Phase 6 Certification IPC envelope (Rust param name = `request`)", () => {
  beforeEach(() => {
    invokeMock.mockClear();
    invokeMock.mockResolvedValue(undefined);
    saveMock.mockClear();
    saveMock.mockReset();
    writeFileMock.mockClear();
    writeFileMock.mockResolvedValue(undefined);
  });

  it("listAchievements invokes `list_achievements_for_learner` with NO payload", async () => {
    invokeMock.mockResolvedValueOnce([]);
    await commands.listAchievements();
    expect(invokeMock).toHaveBeenCalledWith("list_achievements_for_learner");
    expect(invokeMock.mock.calls[0]).toHaveLength(1);
  });

  it("getTrackCertifications sends { request: { trackId } }", async () => {
    invokeMock.mockResolvedValueOnce({
      earnedLevels: [],
      nextLevel: "Associate",
      criteria: "25% of modules mastered",
    });
    await commands.getTrackCertifications({ trackId: "trk-1" });
    expect(invokeMock).toHaveBeenCalledWith("get_track_certifications", {
      request: { trackId: "trk-1" },
    });
  });

  it("exportCertificate sends { request: { achievementId } } and writes via dialog+fs", async () => {
    invokeMock.mockResolvedValueOnce([0x25, 0x50, 0x44, 0x46]); // %PDF
    saveMock.mockResolvedValueOnce("/tmp/cert.pdf");
    const path = await commands.exportCertificate(
      { achievementId: "ach-1" },
      "skillcoco-cert.pdf",
    );
    expect(invokeMock).toHaveBeenCalledWith("export_certificate", {
      request: { achievementId: "ach-1" },
    });
    expect(saveMock).toHaveBeenCalledWith({
      defaultPath: "skillcoco-cert.pdf",
      filters: [{ name: "PDF Certificate", extensions: ["pdf"] }],
    });
    expect(writeFileMock).toHaveBeenCalledWith(
      "/tmp/cert.pdf",
      expect.any(Uint8Array),
    );
    expect(path).toBe("/tmp/cert.pdf");
  });

  it("exportCertificate returns null and skips writeFile on dialog cancel", async () => {
    invokeMock.mockResolvedValueOnce([0x25, 0x50, 0x44, 0x46]);
    saveMock.mockResolvedValueOnce(null);
    const path = await commands.exportCertificate(
      { achievementId: "ach-1" },
      "skillcoco-cert.pdf",
    );
    expect(path).toBeNull();
    expect(writeFileMock).not.toHaveBeenCalled();
  });

  it("exportBadge sends { request: { achievementId } } and writes via dialog+fs", async () => {
    invokeMock.mockResolvedValueOnce([0x89, 0x50, 0x4e, 0x47]); // PNG magic
    saveMock.mockResolvedValueOnce("/tmp/badge.png");
    const path = await commands.exportBadge(
      { achievementId: "ach-1" },
      "skillcoco-badge.png",
    );
    expect(invokeMock).toHaveBeenCalledWith("export_badge", {
      request: { achievementId: "ach-1" },
    });
    expect(saveMock).toHaveBeenCalledWith({
      defaultPath: "skillcoco-badge.png",
      filters: [{ name: "PNG Badge", extensions: ["png"] }],
    });
    expect(writeFileMock).toHaveBeenCalledWith(
      "/tmp/badge.png",
      expect.any(Uint8Array),
    );
    expect(path).toBe("/tmp/badge.png");
  });
});
