import { describe, it, expect } from "vitest";
import type {
  ModuleBlock,
  BlockType,
  BlockStatus,
  LabSpec,
  LabStep,
  StepCheck,
  LabRuntimeChoice,
  LabProgress,
} from "@/types/learning";

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

  it("BlockType union covers the six block kinds (Phase 03.1 adds lab)", () => {
    // FAILING until 03.1-01b lands the `lab` discriminator (LAB-01).
    const types: BlockType[] = [
      "section",
      "text",
      "callout",
      "quiz",
      "flash_cards",
      "lab",
    ];
    expect(types).toHaveLength(6);
  });

  it("BlockStatus union covers the four generation states", () => {
    const statuses: BlockStatus[] = ["pending", "generating", "ready", "failed"];
    expect(statuses).toHaveLength(4);
  });
});

// ── Phase 03.1 Wave 0 — failing scaffolds for lab type surface ──
describe("Phase 03.1 lab type surface (LAB-01, LAB-04)", () => {
  it("BlockType exhaustiveness — switch over every variant compiles", () => {
    // assertNever makes a missing case a TypeScript compile error. If a
    // future BlockType is added without a matching case, `tsc --noEmit`
    // fails — this is the typesystem-level exhaustiveness gate.
    function assertNever(x: never): never {
      throw new Error(`Unhandled BlockType: ${String(x)}`);
    }
    function describeBlockType(t: BlockType): string {
      switch (t) {
        case "section":
          return "section";
        case "text":
          return "text";
        case "callout":
          return "callout";
        case "quiz":
          return "quiz";
        case "flash_cards":
          return "flash_cards";
        case "lab":
          return "lab";
        default:
          return assertNever(t);
      }
    }
    expect(describeBlockType("lab")).toBe("lab");
  });

  it("StepCheck tagged union — each kind narrows to its allowed fields", () => {
    const cmd: StepCheck = { kind: "command_regex", pattern: "^pod/.+ Running$", matchStderr: false };
    const exit: StepCheck = { kind: "exit_code", expected: 0 };
    const file: StepCheck = { kind: "file_state", path: "deploy.yaml", contains: ["replicas: 3"] };
    const ai: StepCheck = { kind: "ai_judge", criteria: "explains the output", threshold: 0.7 };
    expect(cmd.kind).toBe("command_regex");
    expect(exit.kind).toBe("exit_code");
    expect(file.kind).toBe("file_state");
    expect(ai.kind).toBe("ai_judge");
  });

  it("LabSpec round-trips a representative DevOps lab payload", () => {
    const step: LabStep = {
      id: "s1",
      title: "Inspect a pod",
      prompt: "Run `kubectl get pods` and find the running pod.",
      check: { kind: "command_regex", pattern: "Running" },
      hints: ["Use the command from the prompt.", "Pipe through grep.", "kubectl get pods | grep Running"],
    };
    const spec: LabSpec = {
      slug: "pod-inspect",
      title: "Inspect a Pod",
      estimatedMinutes: 10,
      requiresDocker: true,
      image: "kindest/node:v1.30",
      creates: ["deploy.yaml"],
      steps: [step],
    };
    expect(spec.steps).toHaveLength(1);
    expect(spec.steps[0].hints).toHaveLength(3);
    expect(spec.image).toBe("kindest/node:v1.30");
  });

  it("LabRuntimeChoice union covers the three runtime modes", () => {
    const modes: LabRuntimeChoice[] = ["docker", "hostShell", "autoDetect"];
    expect(modes).toHaveLength(3);
  });

  it("LabProgress preserves practicalMastery dimension separate from BKT", () => {
    const progress: LabProgress = {
      blockId: "blk-lab-1",
      currentStep: 2,
      completedStepIds: ["s1", "s2"],
      lastUpdated: "2026-05-05T00:00:00Z",
      practicalMastery: 0.5,
    };
    expect(progress.practicalMastery).toBe(0.5);
    expect(progress.completedStepIds).toEqual(["s1", "s2"]);
  });
});
