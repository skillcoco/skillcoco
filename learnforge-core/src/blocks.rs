//! Block taxonomy + lifecycle state + persistence trait — Phase 7 Wave 6
//! (07-06).
//!
//! Moved verbatim from `src-tauri/src/db/blocks.rs:1-65` (pre-Wave-6). The
//! `BlockType` enum, `BlockStatus` enum, `ModuleBlock` row struct, and the
//! two string-conversion helpers `block_type_to_str` + `status_to_str` are
//! pure data — no SQL, no FS, no Tauri. They are WASM-portable.
//!
//! ## ⚠ camelCase serde on [`ModuleBlock`] is intentional (IPC boundary)
//!
//! `ModuleBlock` is BOTH the SQLite row type AND the IPC payload type —
//! `commands/blocks.rs` (96.7KB, the most-called IPC surface in the
//! codebase) returns / accepts it across the Tauri boundary. The
//! `#[serde(rename_all = "camelCase")]` derive moves with the struct so
//! the JSON shape stays byte-identical to the pre-Wave-6 contract. This
//! is the established convention for *any* future domain-type that
//! crosses Tauri IPC: the camelCase derive lives on the struct, not at
//! the IPC handler layer.
//!
//! ## `BlockStore` trait (A3 lock — per-module storage trait)
//!
//! Per-block-row CRUD lives behind the [`BlockStore`] trait declared
//! next to the types. The trait surface was enumerated by auditing the
//! existing six `pub fn` CRUD helpers in pre-Wave-6
//! `src-tauri/src/db/blocks.rs:68-185`:
//!
//! | Pre-Wave-6 free fn                                            | [`BlockStore`] method     |
//! |---------------------------------------------------------------|---------------------------|
//! | `insert_block(&Connection, &ModuleBlock) -> Result<()>`       | [`BlockStore::insert`]    |
//! | `list_blocks_by_module(&Connection, &str) -> Result<Vec<_>>`  | [`BlockStore::list_for_module`] |
//! | `get_block(&Connection, &str) -> Result<Option<_>>`           | [`BlockStore::get_by_id`] |
//! | `update_block_payload(&Connection, &str, BlockStatus, &str)`  | [`BlockStore::update_payload`] |
//! | `count_blocks_by_module(&Connection, &str) -> Result<i64>`    | [`BlockStore::count_for_module`] |
//! | `delete_blocks_by_module(&Connection, &str) -> Result<usize>` | [`BlockStore::delete_for_module`] |
//!
//! The rusqlite-backed impl lives in
//! `src-tauri/src/storage_impl/blocks.rs` as the local newtype
//! `SqliteBlockStore<'a>(&'a Connection)` — same orphan-rule recipe
//! Waves 2/3/4/5 used.

use serde::{Deserialize, Serialize};

/// Block type taxonomy — serialized as snake_case strings.
///
/// # Variants
///
/// | Variant       | Serialized as |
/// |---------------|---------------|
/// | `Section`     | `"section"`   |
/// | `Text`        | `"text"`      |
/// | `Callout`     | `"callout"`   |
/// | `Quiz`        | `"quiz"`      |
/// | `FlashCards`  | `"flash_cards"` |
/// | `Lab`         | `"lab"` (Phase 03.1 — Hands-on Lab block, LAB-01) |
///
/// # Examples
///
/// ```
/// use learnforge_core::blocks::BlockType;
/// let bt = BlockType::FlashCards;
/// let json = serde_json::to_string(&bt).unwrap();
/// assert_eq!(json, "\"flash_cards\"");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BlockType {
    /// Section header / divider block.
    Section,
    /// Markdown-rendered prose block.
    Text,
    /// Highlighted callout (note / warning / tip).
    Callout,
    /// Multiple-choice quiz block.
    Quiz,
    /// Spaced-repetition flashcards block.
    FlashCards,
    /// Phase 03.1 — Hands-on Lab block (LAB-01).
    Lab,
}

/// Block generation/content status. Drives the async generation pipeline:
/// `Pending → Generating → Ready` (success) or `Pending → Generating → Failed`.
///
/// # Examples
///
/// ```
/// use learnforge_core::blocks::BlockStatus;
/// let st = BlockStatus::Ready;
/// let json = serde_json::to_string(&st).unwrap();
/// assert_eq!(json, "\"ready\"");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BlockStatus {
    /// Initial state — block created, generation not yet started.
    Pending,
    /// Generation in flight (LLM stream / async pipeline running).
    Generating,
    /// Generation completed successfully; `payload_json` populated.
    Ready,
    /// Generation failed; `payload_json` may be empty; `retry_count` ≥ 1.
    Failed,
}

/// Database row for `module_blocks`. **Crosses the Tauri IPC boundary —
/// must use camelCase serde.** See module docs for the convention.
///
/// # Field shape
///
/// Strings hold the serialized lowercase form of the [`BlockType`] /
/// [`BlockStatus`] enums (e.g. `"flash_cards"` / `"ready"`). This matches
/// how the SQLite TEXT columns are populated by the rusqlite adapter,
/// and how the frontend's TypeScript types expect to receive them. Use
/// [`block_type_to_str`] / [`status_to_str`] when constructing rows from
/// typed enum values.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleBlock {
    /// UUID v4 string — primary key.
    pub id: String,
    /// FK → `modules.id`.
    pub module_id: String,
    /// Display order within the module (0-based, ASC).
    pub ordering: i32,
    /// Serialized [`BlockType`] (snake_case variant — see [`block_type_to_str`]).
    pub block_type: String,
    /// Serialized [`BlockStatus`] (snake_case variant — see [`status_to_str`]).
    pub status: String,
    /// JSON-encoded generation params (e.g. quiz difficulty, flashcard count).
    pub params_json: String,
    /// JSON-encoded rendered content payload. Empty until `status = "ready"`.
    pub payload_json: String,
    /// JSON-encoded array of source-document anchors (citations).
    pub source_anchors_json: String,
    /// JSON-encoded metadata blob. Defaults to `{"concept_id":null}` per
    /// the v004 migration's PACK-04 forward-link slot (Phase 2 PATH-02
    /// populates `concept_id` without a schema change).
    pub metadata_json: String,
    /// Number of generation retries attempted (0 on first try).
    pub retry_count: i32,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-update timestamp.
    pub updated_at: String,
}

/// Convert [`BlockStatus`] to its lowercase string representation for DB storage.
///
/// # Examples
///
/// ```
/// use learnforge_core::blocks::{BlockStatus, status_to_str};
/// assert_eq!(status_to_str(&BlockStatus::Ready), "ready");
/// ```
pub fn status_to_str(s: &BlockStatus) -> &'static str {
    match s {
        BlockStatus::Pending => "pending",
        BlockStatus::Generating => "generating",
        BlockStatus::Ready => "ready",
        BlockStatus::Failed => "failed",
    }
}

/// Convert [`BlockType`] to its lowercase string representation for DB storage.
///
/// # Examples
///
/// ```
/// use learnforge_core::blocks::{BlockType, block_type_to_str};
/// assert_eq!(block_type_to_str(&BlockType::FlashCards), "flash_cards");
/// assert_eq!(block_type_to_str(&BlockType::Lab), "lab");
/// ```
pub fn block_type_to_str(t: &BlockType) -> &'static str {
    match t {
        BlockType::Section => "section",
        BlockType::Text => "text",
        BlockType::Callout => "callout",
        BlockType::Quiz => "quiz",
        BlockType::FlashCards => "flash_cards",
        BlockType::Lab => "lab",
    }
}

/// Typed error envelope for [`BlockStore`] operations.
///
/// # Variants
///
/// - `Db(String)` — generic backend / SQL failure (verbatim message preserved).
/// - `NotFound { id }` — block id not present (caller-facing absent semantic).
/// - `InvalidStatus(String)` — DB stored a string that does not deserialize
///   into [`BlockStatus`] (forward-compat / corruption guard).
/// - `InvalidType(String)` — DB stored a string that does not deserialize
///   into [`BlockType`] (forward-compat / corruption guard).
#[derive(Debug, thiserror::Error)]
pub enum BlocksError {
    /// Backend / SQL error — stringified at the trust boundary (T-07-05
    /// pattern, same as `BktError::Db` / `SrError::Db` / `MicrolearningError::Backend`).
    #[error("blocks db error: {0}")]
    Db(String),

    /// Block id not present in the store.
    #[error("block not found: {id}")]
    NotFound {
        /// The block id that was missing.
        id: String,
    },

    /// Stored status string is not a known [`BlockStatus`] variant.
    #[error("invalid block status: {0}")]
    InvalidStatus(String),

    /// Stored block_type string is not a known [`BlockType`] variant.
    #[error("invalid block type: {0}")]
    InvalidType(String),
}

/// Per-block-row CRUD trait. The rusqlite-backed impl lives in
/// `src-tauri/src/storage_impl/blocks.rs`; WASM consumers can implement
/// this against IndexedDB or other browser-portable stores.
///
/// # Trait surface
///
/// The six methods cover every CRUD path enumerated in the pre-Wave-6
/// audit of `src-tauri/src/db/blocks.rs`. Method signatures preserve
/// the existing call shapes so the transitional shim's legacy facades
/// stay zero-friction:
///
/// - [`BlockStore::insert`] / [`BlockStore::list_for_module`] /
///   [`BlockStore::get_by_id`] are simple CRUD reads + writes.
/// - [`BlockStore::update_payload`] mutates two columns (`status` +
///   `payload_json`) atomically — the pre-Wave-6 free fn did the same
///   in one SQL `UPDATE`.
/// - [`BlockStore::count_for_module`] returns a row count.
/// - [`BlockStore::delete_for_module`] removes all blocks belonging to
///   a module (used by `regenerate_module` in 03-03) and returns the
///   number of rows affected.
///
/// # Trait-object safety
///
/// All methods take `&self` and return owned values — the trait is
/// **object-safe** so IPC code that holds a `&dyn BlockStore` works.
pub trait BlockStore {
    /// Insert a block row.
    fn insert(&self, block: &ModuleBlock) -> Result<(), BlocksError>;

    /// List all blocks for a module, ordered by `ordering ASC`.
    fn list_for_module(&self, module_id: &str) -> Result<Vec<ModuleBlock>, BlocksError>;

    /// Fetch a single block by id. Returns `Ok(None)` when absent
    /// (the pre-Wave-6 `get_block` semantic — *not* `Err(NotFound)`).
    fn get_by_id(&self, id: &str) -> Result<Option<ModuleBlock>, BlocksError>;

    /// Advance `status` and replace `payload_json` for a single block.
    /// `updated_at` is bumped to `datetime('now')` by the backend.
    fn update_payload(
        &self,
        id: &str,
        status: BlockStatus,
        payload_json: &str,
    ) -> Result<(), BlocksError>;

    /// Count blocks for a module.
    fn count_for_module(&self, module_id: &str) -> Result<i64, BlocksError>;

    /// Delete all blocks for a module. Returns rows affected.
    /// Used by `regenerate_module` in 03-03.
    fn delete_for_module(&self, module_id: &str) -> Result<usize, BlocksError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Pure type-level tests moved from pre-Wave-6 src-tauri/src/db/blocks.rs ──

    /// Serde test: serialized JSON must contain camelCase keys (IPC contract).
    #[test]
    fn module_block_serializes_camel_case() {
        let block = ModuleBlock {
            id: "blk-001".to_string(),
            module_id: "mod-001".to_string(),
            ordering: 0,
            block_type: "section".to_string(),
            status: "pending".to_string(),
            params_json: "{}".to_string(),
            payload_json: "{}".to_string(),
            source_anchors_json: "[]".to_string(),
            metadata_json: r#"{"concept_id":null}"#.to_string(),
            retry_count: 0,
            created_at: "2026-05-05T00:00:00Z".to_string(),
            updated_at: "2026-05-05T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("moduleId"), "must serialize to moduleId");
        assert!(json.contains("blockType"), "must serialize to blockType");
        assert!(json.contains("paramsJson"), "must serialize to paramsJson");
        assert!(json.contains("payloadJson"), "must serialize to payloadJson");
        assert!(
            json.contains("sourceAnchorsJson"),
            "must serialize to sourceAnchorsJson"
        );
        assert!(
            json.contains("metadataJson"),
            "must serialize to metadataJson"
        );
        assert!(json.contains("retryCount"), "must serialize to retryCount");
        assert!(json.contains("createdAt"), "must serialize to createdAt");
        assert!(json.contains("updatedAt"), "must serialize to updatedAt");
    }

    /// Round-trip: serialize → deserialize → equal field values.
    #[test]
    fn module_block_round_trip() {
        let block = ModuleBlock {
            id: "blk-rt".to_string(),
            module_id: "mod-rt".to_string(),
            ordering: 3,
            block_type: "flash_cards".to_string(),
            status: "ready".to_string(),
            params_json: r#"{"count":5}"#.to_string(),
            payload_json: r#"{"cards":[]}"#.to_string(),
            source_anchors_json: "[]".to_string(),
            metadata_json: r#"{"concept_id":"c-1"}"#.to_string(),
            retry_count: 1,
            created_at: "2026-06-15T00:00:00Z".to_string(),
            updated_at: "2026-06-15T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&block).unwrap();
        let back: ModuleBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, block.id);
        assert_eq!(back.module_id, block.module_id);
        assert_eq!(back.ordering, block.ordering);
        assert_eq!(back.block_type, block.block_type);
        assert_eq!(back.status, block.status);
        assert_eq!(back.retry_count, block.retry_count);
    }

    /// LAB-01 — BlockType::Lab serializes / deserializes as "lab" string.
    #[test]
    fn block_type_lab_serializes_as_lab() {
        let bt = BlockType::Lab;
        let json = serde_json::to_string(&bt).unwrap();
        assert_eq!(json, "\"lab\"", "BlockType::Lab must serialize as \"lab\"");

        let back: BlockType = serde_json::from_str("\"lab\"").unwrap();
        assert_eq!(back, BlockType::Lab, "round-trip back to Lab variant");
    }

    /// LAB-01 — block_type_to_str arm for Lab returns "lab".
    #[test]
    fn block_type_to_str_lab_arm() {
        assert_eq!(block_type_to_str(&BlockType::Lab), "lab");
    }

    /// block_type_to_str covers all six variants.
    #[test]
    fn block_type_to_str_all_variants() {
        assert_eq!(block_type_to_str(&BlockType::Section), "section");
        assert_eq!(block_type_to_str(&BlockType::Text), "text");
        assert_eq!(block_type_to_str(&BlockType::Callout), "callout");
        assert_eq!(block_type_to_str(&BlockType::Quiz), "quiz");
        assert_eq!(block_type_to_str(&BlockType::FlashCards), "flash_cards");
        assert_eq!(block_type_to_str(&BlockType::Lab), "lab");
    }

    /// status_to_str covers all four variants.
    #[test]
    fn status_to_str_all_variants() {
        assert_eq!(status_to_str(&BlockStatus::Pending), "pending");
        assert_eq!(status_to_str(&BlockStatus::Generating), "generating");
        assert_eq!(status_to_str(&BlockStatus::Ready), "ready");
        assert_eq!(status_to_str(&BlockStatus::Failed), "failed");
    }

    /// BlockStatus serde round-trip — all four variants.
    #[test]
    fn block_status_serde_round_trip_all_variants() {
        for (variant, json_lit) in [
            (BlockStatus::Pending, "\"pending\""),
            (BlockStatus::Generating, "\"generating\""),
            (BlockStatus::Ready, "\"ready\""),
            (BlockStatus::Failed, "\"failed\""),
        ] {
            let s = serde_json::to_string(&variant).unwrap();
            assert_eq!(s, json_lit, "serialize {variant:?}");
            let back: BlockStatus = serde_json::from_str(json_lit).unwrap();
            assert_eq!(back, variant, "deserialize {json_lit}");
        }
    }

    /// BlockType serde round-trip — all six variants.
    #[test]
    fn block_type_serde_round_trip_all_variants() {
        for (variant, json_lit) in [
            (BlockType::Section, "\"section\""),
            (BlockType::Text, "\"text\""),
            (BlockType::Callout, "\"callout\""),
            (BlockType::Quiz, "\"quiz\""),
            (BlockType::FlashCards, "\"flash_cards\""),
            (BlockType::Lab, "\"lab\""),
        ] {
            let s = serde_json::to_string(&variant).unwrap();
            assert_eq!(s, json_lit, "serialize {variant:?}");
            let back: BlockType = serde_json::from_str(json_lit).unwrap();
            assert_eq!(back, variant, "deserialize {json_lit}");
        }
    }

    // ── BlockStore trait surface tests ──

    /// BlocksError Display renders all variants.
    #[test]
    fn blocks_error_renders() {
        let e1 = BlocksError::Db("conn closed".to_string());
        assert_eq!(format!("{}", e1), "blocks db error: conn closed");

        let e2 = BlocksError::NotFound {
            id: "blk-x".to_string(),
        };
        assert_eq!(format!("{}", e2), "block not found: blk-x");

        let e3 = BlocksError::InvalidStatus("weird".to_string());
        assert_eq!(format!("{}", e3), "invalid block status: weird");

        let e4 = BlocksError::InvalidType("bogus".to_string());
        assert_eq!(format!("{}", e4), "invalid block type: bogus");
    }

    /// BlockStore is implementable in the host crate — locks the trait
    /// surface against accidental orphan-rule trip-ups (E0117 won't fire
    /// inside this crate because the trait IS defined here).
    struct InMemoryBlockStore {
        rows: std::sync::Mutex<Vec<ModuleBlock>>,
    }

    impl BlockStore for InMemoryBlockStore {
        fn insert(&self, block: &ModuleBlock) -> Result<(), BlocksError> {
            self.rows.lock().unwrap().push(block.clone());
            Ok(())
        }
        fn list_for_module(
            &self,
            module_id: &str,
        ) -> Result<Vec<ModuleBlock>, BlocksError> {
            let mut out: Vec<ModuleBlock> = self
                .rows
                .lock()
                .unwrap()
                .iter()
                .filter(|b| b.module_id == module_id)
                .cloned()
                .collect();
            out.sort_by_key(|b| b.ordering);
            Ok(out)
        }
        fn get_by_id(&self, id: &str) -> Result<Option<ModuleBlock>, BlocksError> {
            Ok(self
                .rows
                .lock()
                .unwrap()
                .iter()
                .find(|b| b.id == id)
                .cloned())
        }
        fn update_payload(
            &self,
            id: &str,
            status: BlockStatus,
            payload_json: &str,
        ) -> Result<(), BlocksError> {
            let mut g = self.rows.lock().unwrap();
            for b in g.iter_mut() {
                if b.id == id {
                    b.status = status_to_str(&status).to_string();
                    b.payload_json = payload_json.to_string();
                    return Ok(());
                }
            }
            Err(BlocksError::NotFound { id: id.to_string() })
        }
        fn count_for_module(&self, module_id: &str) -> Result<i64, BlocksError> {
            Ok(self
                .rows
                .lock()
                .unwrap()
                .iter()
                .filter(|b| b.module_id == module_id)
                .count() as i64)
        }
        fn delete_for_module(&self, module_id: &str) -> Result<usize, BlocksError> {
            let mut g = self.rows.lock().unwrap();
            let before = g.len();
            g.retain(|b| b.module_id != module_id);
            Ok(before - g.len())
        }
    }

    fn sample(id: &str, mod_id: &str, ordering: i32) -> ModuleBlock {
        ModuleBlock {
            id: id.to_string(),
            module_id: mod_id.to_string(),
            ordering,
            block_type: "section".to_string(),
            status: "pending".to_string(),
            params_json: "{}".to_string(),
            payload_json: "{}".to_string(),
            source_anchors_json: "[]".to_string(),
            metadata_json: r#"{"concept_id":null}"#.to_string(),
            retry_count: 0,
            created_at: "2026-06-15T00:00:00Z".to_string(),
            updated_at: "2026-06-15T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn block_store_trait_compiles() {
        let store = InMemoryBlockStore {
            rows: std::sync::Mutex::new(vec![]),
        };
        store.insert(&sample("b1", "m1", 0)).unwrap();
        store.insert(&sample("b2", "m1", 1)).unwrap();
        store.insert(&sample("b3", "m2", 0)).unwrap();

        assert_eq!(store.count_for_module("m1").unwrap(), 2);
        assert_eq!(store.count_for_module("m2").unwrap(), 1);

        let list = store.list_for_module("m1").unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].ordering, 0);
        assert_eq!(list[1].ordering, 1);

        let got = store.get_by_id("b2").unwrap().unwrap();
        assert_eq!(got.module_id, "m1");

        assert!(store.get_by_id("missing").unwrap().is_none());

        store
            .update_payload("b1", BlockStatus::Ready, r#"{"k":"v"}"#)
            .unwrap();
        let after = store.get_by_id("b1").unwrap().unwrap();
        assert_eq!(after.status, "ready");
        assert_eq!(after.payload_json, r#"{"k":"v"}"#);

        let deleted = store.delete_for_module("m1").unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(store.count_for_module("m1").unwrap(), 0);
        assert_eq!(store.count_for_module("m2").unwrap(), 1);
    }

    #[test]
    fn block_store_is_object_safe() {
        let store = InMemoryBlockStore {
            rows: std::sync::Mutex::new(vec![]),
        };
        let dynstore: &dyn BlockStore = &store;
        dynstore.insert(&sample("dyn-1", "m", 0)).unwrap();
        assert_eq!(dynstore.count_for_module("m").unwrap(), 1);
    }
}
