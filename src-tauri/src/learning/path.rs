use serde::{Deserialize, Serialize};

/// A single edge entry from `learning_paths.edges_json`.
/// The edges are stored with camelCase keys (from generate_learning_path),
/// but old rows may use snake_case — the alias handles both.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EdgeRecord {
    pub from: String,
    pub to: String,
    #[serde(rename = "type", alias = "edgeType", alias = "edge_type", default = "default_edge_type")]
    pub edge_type: String,
}

fn default_edge_type() -> String {
    "prerequisite".to_string()
}

/// Parse the `edges_json` TEXT column from `learning_paths` into a Vec of EdgeRecord.
/// Returns `Ok(vec![])` for an empty JSON array.
/// Returns `Err` for malformed JSON.
pub fn parse_edges_json(edges_json: &str) -> Result<Vec<EdgeRecord>, String> {
    serde_json::from_str(edges_json)
        .map_err(|e| format!("Failed to parse edges_json: {}", e))
}

/// Returns `true` if every module that points TO `module_id` (i.e., every prerequisite)
/// has `mastery_level >= MASTERY_THRESHOLD` for the given learner.
///
/// Diamond DAG correctness: ALL incoming edges are checked, not just the most-recently
/// completed one. This prevents premature unlock in A->B, A->C, B->D, C->D topologies.
pub fn all_prerequisites_mastered(
    conn: &rusqlite::Connection,
    learner_id: &str,
    module_id: &str,
    edges: &[EdgeRecord],
) -> Result<bool, String> {
    use crate::learning::adaptive::MASTERY_THRESHOLD;

    let prereqs: Vec<&str> = edges
        .iter()
        .filter(|e| e.to == module_id)
        .map(|e| e.from.as_str())
        .collect();

    if prereqs.is_empty() {
        // No prerequisites → always unlockable
        return Ok(true);
    }

    for prereq_id in &prereqs {
        let mastery: f64 = conn
            .query_row(
                "SELECT mastery_level FROM module_progress WHERE module_id = ?1 AND learner_id = ?2",
                rusqlite::params![prereq_id, learner_id],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        if mastery < MASTERY_THRESHOLD {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Represents a node in the learning path DAG
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PathNode {
    pub id: String,
    pub title: String,
    pub description: String,
    pub module_type: String,
    pub difficulty: i32,
    pub estimated_minutes: i32,
    pub objectives: Vec<String>,
    pub prerequisites: Vec<String>,
}

/// Represents an edge in the learning path DAG
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PathEdge {
    pub from: String,
    pub to: String,
    pub edge_type: String, // "prerequisite", "recommended", "optional"
}

/// Validates that a learning path is a valid DAG (no cycles)
pub fn validate_dag(nodes: &[PathNode], edges: &[PathEdge]) -> Result<(), String> {
    // Topological sort using Kahn's algorithm
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
        Err("Learning path contains a cycle - invalid DAG".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_dag() {
        let nodes = vec![
            PathNode { id: "a".into(), title: "A".into(), description: "".into(),
                module_type: "lesson".into(), difficulty: 1, estimated_minutes: 30,
                objectives: vec![], prerequisites: vec![] },
            PathNode { id: "b".into(), title: "B".into(), description: "".into(),
                module_type: "lesson".into(), difficulty: 2, estimated_minutes: 30,
                objectives: vec![], prerequisites: vec!["a".into()] },
        ];
        let edges = vec![PathEdge { from: "a".into(), to: "b".into(), edge_type: "prerequisite".into() }];
        assert!(validate_dag(&nodes, &edges).is_ok());
    }

    #[test]
    fn test_cycle_detected() {
        let nodes = vec![
            PathNode { id: "a".into(), title: "A".into(), description: "".into(),
                module_type: "lesson".into(), difficulty: 1, estimated_minutes: 30,
                objectives: vec![], prerequisites: vec![] },
            PathNode { id: "b".into(), title: "B".into(), description: "".into(),
                module_type: "lesson".into(), difficulty: 2, estimated_minutes: 30,
                objectives: vec![], prerequisites: vec![] },
        ];
        let edges = vec![
            PathEdge { from: "a".into(), to: "b".into(), edge_type: "prerequisite".into() },
            PathEdge { from: "b".into(), to: "a".into(), edge_type: "prerequisite".into() },
        ];
        assert!(validate_dag(&nodes, &edges).is_err());
    }

    // ── parse_edges_json tests (LOOP-02) ──

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

    // ── all_prerequisites_mastered tests (LOOP-02 diamond) ──

    fn setup_test_db_for_path() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE learner_profiles (id TEXT PRIMARY KEY);
             CREATE TABLE learning_tracks (id TEXT PRIMARY KEY, learner_id TEXT, topic TEXT, domain_module TEXT, goal TEXT);
             CREATE TABLE learning_paths (id TEXT PRIMARY KEY, track_id TEXT, modules_json TEXT DEFAULT '[]', edges_json TEXT DEFAULT '[]', generated_by_model TEXT);
             CREATE TABLE modules (id TEXT PRIMARY KEY, path_id TEXT, title TEXT, ordering INTEGER DEFAULT 0);
             CREATE TABLE module_progress (
                 id TEXT PRIMARY KEY,
                 module_id TEXT NOT NULL,
                 learner_id TEXT NOT NULL,
                 status TEXT NOT NULL DEFAULT 'locked',
                 score REAL,
                 time_spent INTEGER NOT NULL DEFAULT 0,
                 attempts INTEGER NOT NULL DEFAULT 0,
                 mastery_level REAL NOT NULL DEFAULT 0.0,
                 started_at TEXT,
                 completed_at TEXT,
                 UNIQUE(module_id, learner_id)
             );",
        ).unwrap();
        conn
    }

    #[test]
    fn all_prereqs_mastered_linear_chain() {
        let conn = setup_test_db_for_path();
        let learner = "learner-1";

        // Insert module progress: a=mastered, b=locked
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level, status) VALUES ('p1', 'a', ?1, 0.8, 'completed')", [learner]).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level, status) VALUES ('p2', 'b', ?1, 0.0, 'locked')", [learner]).unwrap();

        let edges = vec![
            EdgeRecord { from: "a".into(), to: "b".into(), edge_type: "prerequisite".into() },
        ];

        // b's only prerequisite (a) is mastered → should unlock
        assert!(all_prerequisites_mastered(&conn, learner, "b", &edges).unwrap());
    }

    #[test]
    fn all_prereqs_mastered_diamond_partial() {
        let conn = setup_test_db_for_path();
        let learner = "learner-1";

        // Diamond: a->b, a->c, b->d, c->d
        // a mastered, b mastered, c NOT mastered => d should NOT unlock
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level) VALUES ('p1','a',?1,0.8)", [learner]).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level) VALUES ('p2','b',?1,0.8)", [learner]).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level) VALUES ('p3','c',?1,0.2)", [learner]).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level) VALUES ('p4','d',?1,0.0)", [learner]).unwrap();

        let edges = vec![
            EdgeRecord { from: "a".into(), to: "b".into(), edge_type: "prerequisite".into() },
            EdgeRecord { from: "a".into(), to: "c".into(), edge_type: "prerequisite".into() },
            EdgeRecord { from: "b".into(), to: "d".into(), edge_type: "prerequisite".into() },
            EdgeRecord { from: "c".into(), to: "d".into(), edge_type: "prerequisite".into() },
        ];

        // d has two prereqs (b and c); c is not mastered → false
        assert!(!all_prerequisites_mastered(&conn, learner, "d", &edges).unwrap());
    }

    #[test]
    fn all_prereqs_mastered_diamond_complete() {
        let conn = setup_test_db_for_path();
        let learner = "learner-1";

        // Diamond complete: both b and c mastered → d should unlock
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level) VALUES ('p1','b',?1,0.8)", [learner]).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level) VALUES ('p2','c',?1,0.75)", [learner]).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level) VALUES ('p3','d',?1,0.0)", [learner]).unwrap();

        let edges = vec![
            EdgeRecord { from: "b".into(), to: "d".into(), edge_type: "prerequisite".into() },
            EdgeRecord { from: "c".into(), to: "d".into(), edge_type: "prerequisite".into() },
        ];

        assert!(all_prerequisites_mastered(&conn, learner, "d", &edges).unwrap());
    }
}
