use ruvector_core::types::{DbOptions, DistanceMetric, VectorEntry, SearchQuery};
use ruvector_core::VectorDB;
use ruvector_graph::{GraphDB, NodeBuilder, EdgeBuilder};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Manages concept embeddings for semantic search across learning content.
pub struct VectorStore {
    pub db: Arc<Mutex<VectorDB>>,
}

impl VectorStore {
    pub fn new(storage_path: &str) -> Result<Self, String> {
        let mut options = DbOptions::default();
        options.dimensions = 384; // all-MiniLM-L6-v2 or provider embeddings
        options.distance_metric = DistanceMetric::Cosine;
        options.storage_path = storage_path.to_string();

        let db = VectorDB::new(options).map_err(|e| format!("VectorDB init failed: {}", e))?;
        Ok(Self {
            db: Arc::new(Mutex::new(db)),
        })
    }

    pub async fn index_concept(
        &self,
        id: &str,
        embedding: Vec<f32>,
        metadata: serde_json::Value,
    ) -> Result<String, String> {
        let db = self.db.lock().await;
        let entry = VectorEntry {
            id: Some(id.to_string()),
            vector: embedding,
            metadata: Some(
                serde_json::from_value(metadata).unwrap_or_default(),
            ),
        };
        db.insert(entry).map_err(|e| format!("Insert failed: {}", e))
    }

    pub async fn search_similar(
        &self,
        query_embedding: Vec<f32>,
        k: usize,
    ) -> Result<Vec<(String, f32)>, String> {
        let db = self.db.lock().await;
        let results = db
            .search(SearchQuery {
                vector: query_embedding,
                k,
                filter: None,
                ef_search: None,
            })
            .map_err(|e| format!("Search failed: {}", e))?;

        Ok(results.into_iter().map(|r| (r.id, r.score)).collect())
    }
}

/// Manages learning path DAGs as a proper graph structure.
pub struct LearningGraph {
    pub graph: Arc<Mutex<GraphDB>>,
}

impl LearningGraph {
    pub fn new() -> Self {
        Self {
            graph: Arc::new(Mutex::new(GraphDB::new())),
        }
    }

    /// Store a learning path DAG from AI-generated modules and edges.
    pub async fn store_path(
        &self,
        path_id: &str,
        modules: &[serde_json::Value],
        edges: &[serde_json::Value],
    ) -> Result<(), String> {
        let graph = self.graph.lock().await;

        // Create module nodes
        for module in modules {
            let id = module["id"].as_str().unwrap_or("unknown");
            let node = NodeBuilder::new()
                .id(format!("{}:{}", path_id, id))
                .label("Module")
                .property("title", module["title"].as_str().unwrap_or(""))
                .property("difficulty", module["difficulty"].as_i64().unwrap_or(1))
                .property("path_id", path_id)
                .build();

            graph.create_node(node).map_err(|e| e.to_string())?;
        }

        // Create prerequisite edges
        for edge in edges {
            let from = edge["from"].as_str().unwrap_or("");
            let to = edge["to"].as_str().unwrap_or("");

            let e = EdgeBuilder::new(
                format!("{}:{}", path_id, from),
                format!("{}:{}", path_id, to),
                "PREREQUISITE",
            )
            .build();

            graph.create_edge(e).map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    /// Get prerequisite module IDs for a given module.
    pub async fn get_prerequisites(&self, path_id: &str, module_id: &str) -> Result<Vec<String>, String> {
        let graph = self.graph.lock().await;
        let full_id = format!("{}:{}", path_id, module_id);

        let incoming = graph.get_incoming_edges(&full_id);
        Ok(incoming
            .iter()
            .map(|e| e.from.strip_prefix(&format!("{}:", path_id)).unwrap_or(&e.from).to_string())
            .collect())
    }

    /// Get modules that depend on a given module (unlocked when this one completes).
    pub async fn get_dependents(&self, path_id: &str, module_id: &str) -> Result<Vec<String>, String> {
        let graph = self.graph.lock().await;
        let full_id = format!("{}:{}", path_id, module_id);

        let outgoing = graph.get_outgoing_edges(&full_id);
        Ok(outgoing
            .iter()
            .map(|e| e.to.strip_prefix(&format!("{}:", path_id)).unwrap_or(&e.to).to_string())
            .collect())
    }
}

/// Combined vector + graph state managed by Tauri.
pub struct VectorState {
    pub vectors: VectorStore,
    pub graph: LearningGraph,
}

impl VectorState {
    pub fn new(storage_path: &str) -> Result<Self, String> {
        Ok(Self {
            vectors: VectorStore::new(storage_path)?,
            graph: LearningGraph::new(),
        })
    }
}
