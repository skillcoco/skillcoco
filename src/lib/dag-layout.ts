/**
 * DAG Layout Engine
 *
 * Computes a column-based layout for a directed acyclic graph of modules.
 * Each module is assigned a depth (column) based on its longest path from
 * any root node, then positioned vertically within that column.
 */

import type { PathModule, PathEdge } from "@/types";

export interface PositionedModule {
  module: PathModule;
  /** Column index (0 = leftmost / root nodes) */
  depth: number;
  /** Row index within the column */
  row: number;
  /** Pixel x coordinate (center of node) */
  x: number;
  /** Pixel y coordinate (center of node) */
  y: number;
}

export interface LayoutEdge {
  fromId: string;
  toId: string;
  fromX: number;
  fromY: number;
  toX: number;
  toY: number;
  type: PathEdge["type"];
}

export interface DAGLayout {
  nodes: PositionedModule[];
  edges: LayoutEdge[];
  /** Total width needed for the layout */
  width: number;
  /** Total height needed for the layout */
  height: number;
  /** Number of depth columns */
  columns: number;
}

/** Layout configuration */
const NODE_WIDTH = 240;
const NODE_HEIGHT = 100;
const COLUMN_GAP = 160;
const ROW_GAP = 40;
const PADDING_X = 40;
const PADDING_Y = 40;

/**
 * Compute topological depth for each node using longest-path from roots.
 * This ensures that a node always appears after ALL of its prerequisites.
 */
function computeDepths(
  modules: PathModule[],
  edges: PathEdge[],
): Map<string, number> {
  const ids = new Set(modules.map((m) => m.id));
  const inEdges = new Map<string, string[]>();
  const outEdges = new Map<string, string[]>();

  for (const id of ids) {
    inEdges.set(id, []);
    outEdges.set(id, []);
  }

  for (const edge of edges) {
    if (ids.has(edge.from) && ids.has(edge.to)) {
      outEdges.get(edge.from)!.push(edge.to);
      inEdges.get(edge.to)!.push(edge.from);
    }
  }

  // BFS from roots using Kahn's algorithm, tracking longest path
  const depth = new Map<string, number>();
  const inDegree = new Map<string, number>();

  for (const id of ids) {
    inDegree.set(id, inEdges.get(id)!.length);
  }

  const queue: string[] = [];
  for (const id of ids) {
    if (inDegree.get(id) === 0) {
      queue.push(id);
      depth.set(id, 0);
    }
  }

  while (queue.length > 0) {
    const current = queue.shift()!;
    const currentDepth = depth.get(current)!;

    for (const next of outEdges.get(current)!) {
      const existing = depth.get(next) ?? -1;
      depth.set(next, Math.max(existing, currentDepth + 1));

      const remaining = inDegree.get(next)! - 1;
      inDegree.set(next, remaining);
      if (remaining === 0) {
        queue.push(next);
      }
    }
  }

  // Handle any nodes not reached (disconnected) -- assign depth 0
  for (const id of ids) {
    if (!depth.has(id)) {
      depth.set(id, 0);
    }
  }

  return depth;
}

/**
 * Lay out a DAG of modules into positioned nodes and edges.
 */
export function layoutDAG(
  modules: PathModule[],
  edges: PathEdge[],
): DAGLayout {
  if (modules.length === 0) {
    return { nodes: [], edges: [], width: 0, height: 0, columns: 0 };
  }

  const depthMap = computeDepths(modules, edges);

  // Group modules by depth column
  const columns = new Map<number, PathModule[]>();
  let maxDepth = 0;

  for (const mod of modules) {
    const d = depthMap.get(mod.id) ?? 0;
    maxDepth = Math.max(maxDepth, d);
    if (!columns.has(d)) columns.set(d, []);
    columns.get(d)!.push(mod);
  }

  // Sort modules within each column by difficulty for visual consistency
  for (const [, col] of columns) {
    col.sort((a, b) => a.difficulty - b.difficulty);
  }

  // Position nodes
  const positionedNodes: PositionedModule[] = [];
  const nodePositions = new Map<string, { x: number; y: number }>();

  // Find max column height to center shorter columns
  let maxColumnHeight = 0;
  for (const [, col] of columns) {
    const h = col.length * NODE_HEIGHT + (col.length - 1) * ROW_GAP;
    maxColumnHeight = Math.max(maxColumnHeight, h);
  }

  for (let depth = 0; depth <= maxDepth; depth++) {
    const col = columns.get(depth) ?? [];
    const columnHeight = col.length * NODE_HEIGHT + (col.length - 1) * ROW_GAP;
    const yOffset = (maxColumnHeight - columnHeight) / 2;

    for (let row = 0; row < col.length; row++) {
      const mod = col[row];
      const x = PADDING_X + depth * (NODE_WIDTH + COLUMN_GAP) + NODE_WIDTH / 2;
      const y = PADDING_Y + yOffset + row * (NODE_HEIGHT + ROW_GAP) + NODE_HEIGHT / 2;

      nodePositions.set(mod.id, { x, y });
      positionedNodes.push({
        module: mod,
        depth,
        row,
        x,
        y,
      });
    }
  }

  // Build layout edges
  const layoutEdges: LayoutEdge[] = [];
  for (const edge of edges) {
    const fromPos = nodePositions.get(edge.from);
    const toPos = nodePositions.get(edge.to);
    if (fromPos && toPos) {
      layoutEdges.push({
        fromId: edge.from,
        toId: edge.to,
        fromX: fromPos.x + NODE_WIDTH / 2, // right edge of from node
        fromY: fromPos.y,
        toX: toPos.x - NODE_WIDTH / 2, // left edge of to node
        toY: toPos.y,
        type: edge.type,
      });
    }
  }

  const totalWidth =
    PADDING_X * 2 + (maxDepth + 1) * NODE_WIDTH + maxDepth * COLUMN_GAP;
  const totalHeight = PADDING_Y * 2 + maxColumnHeight;

  return {
    nodes: positionedNodes,
    edges: layoutEdges,
    width: totalWidth,
    height: totalHeight,
    columns: maxDepth + 1,
  };
}

/** Exported constants for use in rendering */
export const DAG_NODE_WIDTH = NODE_WIDTH;
export const DAG_NODE_HEIGHT = NODE_HEIGHT;
