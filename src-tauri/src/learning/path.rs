use serde::{Deserialize, Serialize};

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
}
