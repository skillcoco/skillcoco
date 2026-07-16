//! Learning-path DAG primitives — pure (no I/O) pieces shared by every host.
//!
//! Moved from `src-tauri/src/learning/path.rs` in Phase 7 Wave 2 (Pitfall 8 —
//! mixed pure/DB code split). The DB-touching prerequisite check
//! ([`all_prerequisites_mastered`]) is preserved as a free function that takes
//! a [`BktStore`] trait object, so persistence stays
//! behind the storage trait and `skillcoco-core` itself remains
//! WASM-portable / rusqlite-free.
//!
//! ## Surface
//!
//! - [`EdgeRecord`] — edge row as persisted in `learning_paths.edges_json`
//!   (camelCase serde + snake_case alias for legacy rows).
//! - [`PathNode`] / [`PathEdge`] — in-memory DAG types used by the path
//!   generator.
//! - [`parse_edges_json`] — JSON → `Vec<EdgeRecord>` (with default + alias
//!   handling).
//! - [`validate_dag`] — Kahn's algorithm cycle check.
//! - [`all_prerequisites_mastered`] — trait-driven prerequisite gate (uses
//!   any [`BktStore`] implementation; correctness on
//!   diamond DAGs preserved).

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::bkt::{BktError, BktStore, MASTERY_THRESHOLD};

/// Errors returned by the pure path-DAG primitives.
///
/// Stringified at the trust boundary; the host crate's shim downgrades these
/// to `String` to preserve legacy call-site error types until the cleanup
/// wave (07-10) lands.
#[derive(Debug, Error)]
pub enum PathError {
    /// `edges_json` text column failed to parse as JSON or did not match the
    /// expected schema.
    #[error("failed to parse edges_json: {0}")]
    InvalidEdgesJson(String),
    /// The provided node + edge set contains a cycle (no topological order
    /// exists).
    #[error("learning path contains a cycle — invalid DAG")]
    CycleDetected,
}

/// A single edge entry from `learning_paths.edges_json`.
///
/// The edges are stored with camelCase keys (from `generate_learning_path`),
/// but old rows may use `edge_type` snake_case — the alias handles both.
/// Missing `type` field defaults to `"prerequisite"` so the generator can
/// emit minimal `{from, to}` JSON.
///
/// # Example
///
/// ```
/// use skillcoco_core::path::{EdgeRecord, parse_edges_json};
///
/// let json = r#"[{"from":"a","to":"b","type":"prerequisite"}]"#;
/// let edges: Vec<EdgeRecord> = parse_edges_json(json).unwrap();
/// assert_eq!(edges[0].from, "a");
/// assert_eq!(edges[0].edge_type, "prerequisite");
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EdgeRecord {
    /// Source module id.
    pub from: String,
    /// Destination module id.
    pub to: String,
    /// Edge classification (e.g. `"prerequisite"`, `"recommended"`, `"optional"`).
    #[serde(
        rename = "type",
        alias = "edgeType",
        alias = "edge_type",
        default = "default_edge_type"
    )]
    pub edge_type: String,
}

fn default_edge_type() -> String {
    "prerequisite".to_string()
}

/// Parse the `edges_json` TEXT column from `learning_paths` into a Vec of
/// [`EdgeRecord`].
///
/// Returns `Ok(vec![])` for an empty JSON array. Returns
/// [`PathError::InvalidEdgesJson`] for malformed input.
///
/// # Example
///
/// ```
/// use skillcoco_core::path::parse_edges_json;
///
/// // Empty array parses to empty Vec
/// assert!(parse_edges_json("[]").unwrap().is_empty());
/// // Malformed JSON returns Err
/// assert!(parse_edges_json("{not json}").is_err());
/// ```
pub fn parse_edges_json(edges_json: &str) -> Result<Vec<EdgeRecord>, PathError> {
    serde_json::from_str(edges_json)
        .map_err(|e| PathError::InvalidEdgesJson(e.to_string()))
}

/// Returns `true` if every module that points TO `module_id` (i.e., every
/// prerequisite) has `mastery_level >= MASTERY_THRESHOLD` for the given
/// learner.
///
/// **Diamond DAG correctness:** ALL incoming edges are checked, not just the
/// most-recently completed one. This prevents premature unlock in
/// `A->B, A->C, B->D, C->D` topologies.
///
/// Errors:
/// - Returns `Err` only if the store itself errors (excluding
///   [`BktError::NotFound`] which is treated as `mastery = 0.0`, matching
///   the legacy `.unwrap_or(0.0)` behavior).
///
/// # Example
///
/// ```
/// use skillcoco_core::bkt::{BktError, BktStore};
/// use skillcoco_core::path::{EdgeRecord, all_prerequisites_mastered};
///
/// struct AllMastered;
/// impl BktStore for AllMastered {
///     fn read_mastery(&self, _: &str, _: &str) -> Result<f64, BktError> {
///         Ok(0.9)
///     }
/// }
///
/// let edges = vec![
///     EdgeRecord { from: "a".into(), to: "b".into(), edge_type: "prerequisite".into() },
/// ];
/// // a->b; a is mastered → b can unlock
/// assert!(all_prerequisites_mastered(&AllMastered, "learner-1", "b", &edges).unwrap());
/// ```
pub fn all_prerequisites_mastered<S: BktStore>(
    store: &S,
    learner_id: &str,
    module_id: &str,
    edges: &[EdgeRecord],
) -> Result<bool, BktError> {
    let prereqs: Vec<&str> = edges
        .iter()
        .filter(|e| e.to == module_id)
        .map(|e| e.from.as_str())
        .collect();

    if prereqs.is_empty() {
        // No prerequisites → always unlockable.
        return Ok(true);
    }

    for prereq_id in &prereqs {
        // Match legacy semantics: NotFound was previously absorbed by
        // `.unwrap_or(0.0)` in src-tauri/src/learning/path.rs:58 — keep that
        // behavior so the per-store ergonomics don't change.
        let mastery = match store.read_mastery(learner_id, prereq_id) {
            Ok(v) => v,
            Err(BktError::NotFound { .. }) => 0.0,
            Err(other) => return Err(other),
        };

        if mastery < MASTERY_THRESHOLD {
            return Ok(false);
        }
    }

    Ok(true)
}

/// In-memory representation of a node in the learning path DAG.
///
/// Mirrors the wire shape emitted by `commands::ai::generate_learning_path`
/// and consumed by frontend rendering. Pure data — no methods beyond
/// serde + clone.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PathNode {
    /// Module id (stable across regenerations within a track).
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Long-form description.
    pub description: String,
    /// Block taxonomy tag (e.g. `"lesson"`, `"quiz"`).
    pub module_type: String,
    /// Difficulty 1–5.
    pub difficulty: i32,
    /// Estimated time-to-complete in minutes.
    pub estimated_minutes: i32,
    /// Learning objectives.
    pub objectives: Vec<String>,
    /// Prerequisite module ids (for path-renderer hints; the authoritative
    /// edges live in `learning_paths.edges_json`).
    pub prerequisites: Vec<String>,
}

/// Edge in the learning path DAG used by the generator and DAG validator.
///
/// `edge_type` is one of `"prerequisite"`, `"recommended"`, `"optional"`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PathEdge {
    /// Source module id.
    pub from: String,
    /// Destination module id.
    pub to: String,
    /// Edge classification.
    pub edge_type: String,
}

/// Validates that a learning path is a valid DAG (no cycles).
///
/// Uses Kahn's algorithm (topological sort by repeatedly removing
/// zero-in-degree nodes). Returns [`PathError::CycleDetected`] when any
/// nodes remain after the queue drains.
///
/// # Example
///
/// ```
/// use skillcoco_core::path::{PathNode, PathEdge, validate_dag};
///
/// let nodes = vec![
///     PathNode {
///         id: "a".into(), title: "A".into(), description: "".into(),
///         module_type: "lesson".into(), difficulty: 1, estimated_minutes: 30,
///         objectives: vec![], prerequisites: vec![],
///     },
///     PathNode {
///         id: "b".into(), title: "B".into(), description: "".into(),
///         module_type: "lesson".into(), difficulty: 2, estimated_minutes: 30,
///         objectives: vec![], prerequisites: vec!["a".into()],
///     },
/// ];
/// let edges = vec![PathEdge {
///     from: "a".into(), to: "b".into(), edge_type: "prerequisite".into(),
/// }];
/// assert!(validate_dag(&nodes, &edges).is_ok());
/// ```
pub fn validate_dag(nodes: &[PathNode], edges: &[PathEdge]) -> Result<(), PathError> {
    let node_ids: std::collections::HashSet<&str> =
        nodes.iter().map(|n| n.id.as_str()).collect();

    let mut in_degree: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut adj: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();

    for id in &node_ids {
        in_degree.insert(id, 0);
        adj.insert(id, Vec::new());
    }

    for edge in edges {
        if let Some(deg) = in_degree.get_mut(edge.to.as_str()) {
            *deg += 1;
        }
        if let Some(neighbors) = adj.get_mut(edge.from.as_str()) {
            neighbors.push(&edge.to);
        }
    }

    let mut queue: Vec<&str> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&id, _)| id)
        .collect();

    let mut visited = 0;

    while let Some(node) = queue.pop() {
        visited += 1;
        if let Some(neighbors) = adj.get(node) {
            for &neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(neighbor) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push(neighbor);
                    }
                }
            }
        }
    }

    if visited == node_ids.len() {
        Ok(())
    } else {
        Err(PathError::CycleDetected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bkt::{BktError, BktStore};
    use std::collections::HashMap;

    // ── validate_dag tests ──────────────────────────────────────────

    #[test]
    fn test_valid_dag() {
        let nodes = vec![
            PathNode {
                id: "a".into(),
                title: "A".into(),
                description: "".into(),
                module_type: "lesson".into(),
                difficulty: 1,
                estimated_minutes: 30,
                objectives: vec![],
                prerequisites: vec![],
            },
            PathNode {
                id: "b".into(),
                title: "B".into(),
                description: "".into(),
                module_type: "lesson".into(),
                difficulty: 2,
                estimated_minutes: 30,
                objectives: vec![],
                prerequisites: vec!["a".into()],
            },
        ];
        let edges = vec![PathEdge {
            from: "a".into(),
            to: "b".into(),
            edge_type: "prerequisite".into(),
        }];
        assert!(validate_dag(&nodes, &edges).is_ok());
    }

    #[test]
    fn test_cycle_detected() {
        let nodes = vec![
            PathNode {
                id: "a".into(),
                title: "A".into(),
                description: "".into(),
                module_type: "lesson".into(),
                difficulty: 1,
                estimated_minutes: 30,
                objectives: vec![],
                prerequisites: vec![],
            },
            PathNode {
                id: "b".into(),
                title: "B".into(),
                description: "".into(),
                module_type: "lesson".into(),
                difficulty: 2,
                estimated_minutes: 30,
                objectives: vec![],
                prerequisites: vec![],
            },
        ];
        let edges = vec![
            PathEdge {
                from: "a".into(),
                to: "b".into(),
                edge_type: "prerequisite".into(),
            },
            PathEdge {
                from: "b".into(),
                to: "a".into(),
                edge_type: "prerequisite".into(),
            },
        ];
        assert!(validate_dag(&nodes, &edges).is_err());
    }

    // ── parse_edges_json tests ──────────────────────────────────────

    #[test]
    fn parse_edges_valid() {
        let json = r#"[{"from":"a","to":"b","type":"prerequisite"}]"#;
        let edges = parse_edges_json(json).expect("should parse valid JSON");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from, "a");
        assert_eq!(edges[0].to, "b");
        assert_eq!(edges[0].edge_type, "prerequisite");
    }

    #[test]
    fn parse_edges_empty_array() {
        let edges = parse_edges_json("[]").expect("empty array should be Ok");
        assert!(edges.is_empty());
    }

    #[test]
    fn parse_edges_malformed_returns_err() {
        assert!(parse_edges_json("{not json}").is_err());
    }

    #[test]
    fn parse_edges_snake_case_alias() {
        // Old rows may have been stored with edge_type key (snake_case)
        let json = r#"[{"from":"a","to":"b","edge_type":"prerequisite"}]"#;
        let edges = parse_edges_json(json).expect("snake_case edge_type should be accepted via alias");
        assert_eq!(edges[0].edge_type, "prerequisite");
    }

    #[test]
    fn parse_edges_missing_type_gets_default() {
        // Edges from generate_learning_path don't include a type field — default to "prerequisite"
        let json = r#"[{"from":"a","to":"b"}]"#;
        let edges = parse_edges_json(json).expect("missing type key should use default");
        assert_eq!(edges[0].edge_type, "prerequisite");
    }

    // ── all_prerequisites_mastered tests (trait-driven, no DB) ──────

    /// Pure-Rust BktStore stub: per-(learner, module) mastery map.
    /// Used by the Wave 2 prereq-check tests to avoid pulling rusqlite into
    /// skillcoco-core just to verify the algorithm.
    struct MapStore {
        masteries: HashMap<(String, String), f64>,
    }

    impl MapStore {
        fn new() -> Self {
            Self {
                masteries: HashMap::new(),
            }
        }

        fn set(&mut self, learner: &str, module: &str, mastery: f64) -> &mut Self {
            self.masteries
                .insert((learner.to_string(), module.to_string()), mastery);
            self
        }
    }

    impl BktStore for MapStore {
        fn read_mastery(&self, learner_id: &str, module_id: &str) -> Result<f64, BktError> {
            self.masteries
                .get(&(learner_id.to_string(), module_id.to_string()))
                .copied()
                .ok_or_else(|| BktError::NotFound {
                    learner_id: learner_id.to_string(),
                    module_id: module_id.to_string(),
                })
        }
    }

    #[test]
    fn all_prerequisites_mastered_with_stub_store_passes_when_all_above_threshold() {
        // Diamond complete: a->b, a->c, b->d, c->d — all upstream mastered.
        let mut store = MapStore::new();
        store
            .set("L", "a", 0.85)
            .set("L", "b", 0.8)
            .set("L", "c", 0.75);

        let edges = vec![
            EdgeRecord {
                from: "a".into(),
                to: "b".into(),
                edge_type: "prerequisite".into(),
            },
            EdgeRecord {
                from: "a".into(),
                to: "c".into(),
                edge_type: "prerequisite".into(),
            },
            EdgeRecord {
                from: "b".into(),
                to: "d".into(),
                edge_type: "prerequisite".into(),
            },
            EdgeRecord {
                from: "c".into(),
                to: "d".into(),
                edge_type: "prerequisite".into(),
            },
        ];

        // d has two prereqs (b, c); both mastered → unlock.
        assert!(all_prerequisites_mastered(&store, "L", "d", &edges).unwrap());
    }

    #[test]
    fn all_prerequisites_mastered_with_stub_store_fails_when_any_below_threshold() {
        // Diamond partial: c not mastered (0.5 < 0.7) — d should NOT unlock.
        let mut store = MapStore::new();
        store
            .set("L", "a", 0.85)
            .set("L", "b", 0.8)
            .set("L", "c", 0.5); // below MASTERY_THRESHOLD

        let edges = vec![
            EdgeRecord {
                from: "b".into(),
                to: "d".into(),
                edge_type: "prerequisite".into(),
            },
            EdgeRecord {
                from: "c".into(),
                to: "d".into(),
                edge_type: "prerequisite".into(),
            },
        ];

        assert!(!all_prerequisites_mastered(&store, "L", "d", &edges).unwrap());
    }

    #[test]
    fn all_prerequisites_mastered_no_prereqs_returns_true() {
        // module "root" has no incoming edges → unlockable.
        let store = MapStore::new();
        let edges = vec![EdgeRecord {
            from: "a".into(),
            to: "b".into(),
            edge_type: "prerequisite".into(),
        }];
        assert!(all_prerequisites_mastered(&store, "L", "root", &edges).unwrap());
    }

    #[test]
    fn all_prerequisites_mastered_missing_row_treated_as_zero() {
        // Legacy parity: src-tauri/src/learning/path.rs used `.unwrap_or(0.0)`
        // for missing rows, so a NotFound prereq means "not mastered" → false.
        let store = MapStore::new(); // empty — every lookup is NotFound
        let edges = vec![EdgeRecord {
            from: "a".into(),
            to: "b".into(),
            edge_type: "prerequisite".into(),
        }];
        // b has prereq a; a is missing → treat as 0.0 → not mastered → false.
        assert!(!all_prerequisites_mastered(&store, "L", "b", &edges).unwrap());
    }
}
