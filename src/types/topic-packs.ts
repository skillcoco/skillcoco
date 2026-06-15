/**
 * Topic Packs (Phase 5) — TypeScript types matching the Rust serde shape.
 *
 * # Naming
 *
 * - The outer [`TopicPack`] envelope uses **camelCase** (Q9 lock — matches
 *   Rust `LoadedPack`'s `#[serde(rename_all = "camelCase")]`).
 * - The nested [`Pack`] / [`PackModule`] / [`PackEdge`] objects use
 *   **snake_case** field names because the underlying Rust structs hold
 *   the on-disk `pack.json` shape verbatim (no `rename_all` — see
 *   05-02-SUMMARY.md Rule 1 deviation for the rationale).
 *
 * Request structs (`SetTopicPackEnabledRequest`,
 * `GetTopicPackModulesRequest`) and the `PackModulesResult` envelope are
 * camelCase because they cross the IPC boundary.
 */

// ── On-disk pack shape ───────────────────────────────────────────────────

export interface PackModule {
  id: string;
  title: string;
  description: string;
  difficulty?: number | null;
  estimated_minutes?: number | null;
  objectives: string[];
  exercise_types?: string[];
}

export interface PackEdge {
  from: string;
  to: string;
}

export interface Pack {
  id: string;
  title: string;
  description: string;
  domain_module: string;
  estimated_hours?: number | null;
  pack_version: string;
  requires_docker: boolean;
  modules: PackModule[];
  edges: PackEdge[];
}

// ── Loader-side metadata ─────────────────────────────────────────────────

export type PackSource = "bundled" | "skill";

export type ValidationStatus = "ok" | "warnings" | "errors";

/**
 * In-memory pack record returned by the `list_topic_packs` /
 * `list_topic_packs_admin` IPCs. Matches Rust `LoadedPack` 1:1.
 */
export interface TopicPack {
  pack: Pack;
  source: PackSource;
  enabled: boolean;
  validationStatus: ValidationStatus;
  /** Q4 lock — plain strings; structured records deferred. */
  validationMessages: string[];
  /** RFC3339 timestamp set by the loader at last successful read. */
  lastLoadedAt: string;
}

// ── IPC request / response envelopes ─────────────────────────────────────

export interface SetTopicPackEnabledRequest {
  packId: string;
  enabled: boolean;
}

export interface GetTopicPackModulesRequest {
  packId: string;
}

/** Result of `get_topic_pack_modules` — feeds Wave 4's track-creation flow. */
export interface PackModulesResult {
  modules: PackModule[];
  edges: PackEdge[];
}
