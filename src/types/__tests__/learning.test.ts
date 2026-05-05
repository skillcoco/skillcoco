import { describe, it, expect } from "vitest";
import type { ModuleBlock, BlockType, BlockStatus } from "@/types/learning";

describe("Block types — camelCase IPC contract", () => {
  it("ModuleBlock interface accepts a canonical Rust-serialized JSON sample", () => {
    // This is the canonical output shape a Rust ModuleBlock serializes to with
    // #[serde(rename_all = "camelCase")]. The TS interface must match exactly.
    const raw = {
      id: "blk-001",
      moduleId: "mod-001",
      ordering: 0,
      blockType: "section" as BlockType,
      status: "ready" as BlockStatus,
      paramsJson: '{"lesson_title":"Introduction","word_count_target":1500}',
      payloadJson: '{"markdown":"# Introduction\\nContent here.","word_count":312}',
      sourceAnchorsJson: "[]",
      metadataJson: '{"concept_id":null}',
      retryCount: 0,
      createdAt: "2026-05-05T00:00:00Z",
      updatedAt: "2026-05-05T00:00:00Z",
    };

    // Type-level assertion: assigning raw to ModuleBlock must compile without `as any`.
    const block: ModuleBlock = raw;

    // Runtime assertions: every camelCase field must be present.
    expect(block.id).toBe("blk-001");
    expect(block.moduleId).toBe("mod-001");
    expect(block.blockType).toBe("section");
    expect(block.status).toBe("ready");
    expect(block.paramsJson).toContain("lesson_title");
    expect(block.payloadJson).toContain("markdown");
    expect(block.sourceAnchorsJson).toBe("[]");
    expect(block.metadataJson).toContain("concept_id");
    expect(block.retryCount).toBe(0);
    expect(block.createdAt).toContain("2026");
    expect(block.updatedAt).toContain("2026");
  });

  it("BlockType union covers the five block kinds", () => {
    const types: BlockType[] = ["section", "text", "callout", "quiz", "flash_cards"];
    expect(types).toHaveLength(5);
  });

  it("BlockStatus union covers the four generation states", () => {
    const statuses: BlockStatus[] = ["pending", "generating", "ready", "failed"];
    expect(statuses).toHaveLength(4);
  });
});
