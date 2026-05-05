/**
 * Tauri command-wrapper invocation contract tests.
 *
 * Tauri matches the top-level JS argument key to the Rust parameter name. If a
 * wrapper sends { request: ... } but the Rust handler declares `req: T`, the
 * IPC silently fails. FIX-02 burned this lesson once already; these tests
 * lock in the convention for Phase 3 commands.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";

const invokeMock = vi.fn().mockResolvedValue(undefined);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import * as commands from "@/lib/tauri-commands";

describe("Phase 3 IPC argument-key contract (Rust param name = `req`)", () => {
  beforeEach(() => {
    invokeMock.mockClear();
    invokeMock.mockResolvedValue(undefined);
  });

  it("markLessonComplete sends { req: ... } so Rust mark_lesson_complete(req: ...) deserializes", async () => {
    await commands.markLessonComplete("mod-1", "blk-7");
    expect(invokeMock).toHaveBeenCalledWith("mark_lesson_complete", {
      req: { moduleId: "mod-1", blockId: "blk-7" },
    });
  });

  it("submitQuiz sends { req: ... }", async () => {
    invokeMock.mockResolvedValueOnce({
      passed: true,
      score: 75,
      newMasteryLevel: 0.7,
      newlyUnlockedModuleIds: [],
      cardsCreated: 0,
      review: [],
    });
    await commands.submitQuiz({
      moduleId: "mod-1",
      trackId: "trk-1",
      blockId: "blk-q",
      answers: [{ questionId: "q1", chosenOptionId: "a" }],
    });
    expect(invokeMock).toHaveBeenCalledWith("submit_quiz", {
      req: expect.objectContaining({ moduleId: "mod-1" }),
    });
  });

  it("regenerateLesson sends { req: ... }", async () => {
    invokeMock.mockResolvedValueOnce({});
    await commands.regenerateLesson({ blockId: "blk-7" });
    expect(invokeMock).toHaveBeenCalledWith("regenerate_lesson", {
      req: { blockId: "blk-7" },
    });
  });

  it("regenerateModule sends { req: ... }", async () => {
    invokeMock.mockResolvedValueOnce({ blocks: [] });
    await commands.regenerateModule({ moduleId: "mod-1", trackId: "trk-1" });
    expect(invokeMock).toHaveBeenCalledWith("regenerate_module", {
      req: { moduleId: "mod-1", trackId: "trk-1" },
    });
  });
});
