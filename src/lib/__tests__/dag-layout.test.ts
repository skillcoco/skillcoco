import { describe, it, expect } from "vitest";
import { layoutDAG } from "@/lib/dag-layout";
import type { PathModule, PathEdge } from "@/types";

/** Helper to create a minimal PathModule with required fields. */
function makeModule(id: string, difficulty = 1): PathModule {
  return {
    id,
    title: `Module ${id}`,
    description: "",
    type: "lesson",
    difficulty,
    estimatedMinutes: 30,
    objectives: [],
    prerequisites: [],
  };
}

describe("layoutDAG", () => {
  it("returns an empty layout for zero modules", () => {
    const result = layoutDAG([], []);
    expect(result.nodes).toHaveLength(0);
    expect(result.edges).toHaveLength(0);
    expect(result.width).toBe(0);
    expect(result.height).toBe(0);
    expect(result.columns).toBe(0);
  });

  it("lays out a single node at depth 0", () => {
    const modules = [makeModule("a")];
    const result = layoutDAG(modules, []);

    expect(result.nodes).toHaveLength(1);
    expect(result.nodes[0].depth).toBe(0);
    expect(result.nodes[0].row).toBe(0);
    expect(result.columns).toBe(1);
  });

  it("lays out a linear chain with incrementing depths", () => {
    const modules = [makeModule("a"), makeModule("b"), makeModule("c")];
    const edges: PathEdge[] = [
      { from: "a", to: "b", type: "prerequisite" },
      { from: "b", to: "c", type: "prerequisite" },
    ];

    const result = layoutDAG(modules, edges);

    const depthOf = (id: string) => result.nodes.find((n) => n.module.id === id)!.depth;
    expect(depthOf("a")).toBe(0);
    expect(depthOf("b")).toBe(1);
    expect(depthOf("c")).toBe(2);
    expect(result.columns).toBe(3);
  });

  it("handles diamond-shaped dependencies correctly", () => {
    // A -> B, A -> C, B -> D, C -> D
    const modules = [
      makeModule("a"),
      makeModule("b"),
      makeModule("c"),
      makeModule("d"),
    ];
    const edges: PathEdge[] = [
      { from: "a", to: "b", type: "prerequisite" },
      { from: "a", to: "c", type: "prerequisite" },
      { from: "b", to: "d", type: "prerequisite" },
      { from: "c", to: "d", type: "prerequisite" },
    ];

    const result = layoutDAG(modules, edges);

    const depthOf = (id: string) => result.nodes.find((n) => n.module.id === id)!.depth;
    expect(depthOf("a")).toBe(0);
    expect(depthOf("b")).toBe(1);
    expect(depthOf("c")).toBe(1);
    // D must be after BOTH B and C, so depth 2
    expect(depthOf("d")).toBe(2);
    expect(result.columns).toBe(3);
  });

  it("places disconnected nodes at depth 0", () => {
    const modules = [makeModule("a"), makeModule("b"), makeModule("c")];
    const edges: PathEdge[] = []; // no edges

    const result = layoutDAG(modules, edges);

    for (const node of result.nodes) {
      expect(node.depth).toBe(0);
    }
    // All in one column
    expect(result.columns).toBe(1);
  });

  it("assigns unique row positions to nodes within the same column", () => {
    // Two roots and one dependent
    const modules = [makeModule("a", 1), makeModule("b", 2), makeModule("c", 3)];
    const edges: PathEdge[] = [
      { from: "a", to: "c", type: "prerequisite" },
      { from: "b", to: "c", type: "prerequisite" },
    ];

    const result = layoutDAG(modules, edges);

    // A and B should share depth 0 but have different rows
    const col0 = result.nodes.filter((n) => n.depth === 0);
    expect(col0).toHaveLength(2);
    expect(col0[0].row).not.toBe(col0[1].row);
  });

  it("produces layout edges with valid positions", () => {
    const modules = [makeModule("a"), makeModule("b")];
    const edges: PathEdge[] = [{ from: "a", to: "b", type: "prerequisite" }];

    const result = layoutDAG(modules, edges);

    expect(result.edges).toHaveLength(1);
    const edge = result.edges[0];
    expect(edge.fromId).toBe("a");
    expect(edge.toId).toBe("b");
    expect(edge.type).toBe("prerequisite");
    // From-X should be to the right of to-X (left-to-right layout)
    expect(edge.fromX).toBeLessThan(edge.toX);
  });

  it("computes positive width and height for non-empty graphs", () => {
    const modules = [makeModule("a"), makeModule("b")];
    const edges: PathEdge[] = [{ from: "a", to: "b", type: "prerequisite" }];

    const result = layoutDAG(modules, edges);

    expect(result.width).toBeGreaterThan(0);
    expect(result.height).toBeGreaterThan(0);
  });
});
