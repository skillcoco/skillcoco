//! Phase 11 — Video-enriched lessons backend.
//!
//! Exposes two Tauri IPC commands:
//! - `get_lesson_videos(moduleId)`: lazy fetch + indefinite per-lesson cache.
//! - `refresh_lesson_videos(moduleId)`: clears cached rows and re-discovers.
//!
//! On cache miss with a YouTube Data API v3 key present, `fetch_and_rank_videos`
//! calls `search.list`, filters embeddable-only via `videos.list` (D-07),
//! LLM-ranks candidates (D-08), and persists the top-N above the relevance
//! threshold to `lesson_videos`.
//!
//! On any failure path (no key, quota exceeded, offline, nothing passes
//! threshold) the commands return an empty `LessonVideosResult` — never an
//! error — so the frontend panel cleanly vanishes (D-06/D-09).
//!
//! Security: the YouTube API key is NEVER logged. It is passed to reqwest
//! via `.query(...)` parameters only (not string-interpolated into URLs or
//! log messages). T-11-03 mitigation.

use crate::ai::service::{ai_request, AIServiceRequest, ServiceMessage};
use crate::auth::AuthState;
use crate::commands::ai::extract_json_pub;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Top-N videos to keep after LLM ranking + threshold filtering (D-02/D-08).
const VIDEO_RESULT_LIMIT: usize = 3;

/// Minimum relevance score (0.0–1.0) a video must reach to be persisted
/// and returned. Videos below this score are silently discarded (D-08).
const RELEVANCE_THRESHOLD: f32 = 0.6;

// ── IPC structs ───────────────────────────────────────────────────────────────

/// A single ranked video as returned across the IPC boundary.
///
/// `#[serde(rename_all = "camelCase")]` matches the TypeScript interface
/// `LessonVideo` in `src/types/videos.ts` (T-11-02 camelCase contract).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LessonVideo {
    pub video_id: String,
    pub title: String,
    pub channel_title: String,
    pub relevance_score: f32,
}

/// IPC response envelope for both `get_lesson_videos` and `refresh_lesson_videos`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonVideosResult {
    pub videos: Vec<LessonVideo>,
}

// ── Internal helper types (not crossing IPC) ──────────────────────────────────

/// Candidate from `search.list` before embeddable/ranking filters.
#[derive(Debug, Clone)]
struct SearchCandidate {
    video_id: String,
    title: String,
    channel_title: String,
    description: String,
}

/// Scored output from LLM ranking before threshold filtering.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RankedVideo {
    video_id: String,
    relevance_score: f64,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a YouTube `search.list` query string from the lesson title and
/// the first three objective keywords (D-08).
pub fn build_search_query(lesson_title: &str, objectives: &[String]) -> String {
    let keywords: Vec<&str> = objectives
        .iter()
        .take(3)
        .map(|s| s.as_str())
        .collect();
    if keywords.is_empty() {
        lesson_title.to_string()
    } else {
        format!("{} {}", lesson_title, keywords.join(" "))
    }
}

/// Fetch YouTube search candidates, filter to embeddable-only via `videos.list`,
/// LLM-rank them against the lesson title + objectives, apply the relevance
/// threshold, and return the top-`VIDEO_RESULT_LIMIT` results.
///
/// Returns `Err` only for unrecoverable internal errors (HTTP failure, JSON
/// parse failure). The caller converts `Err` to an empty list (D-09).
///
/// **Security:** `youtube_key` is never logged or string-interpolated into
/// URLs — it is always passed as a `reqwest` query parameter (T-11-03).
pub async fn fetch_and_rank_videos(
    youtube_key: &str,
    lesson_title: &str,
    objectives: &[String],
    llm_auth: &AuthState,
) -> Result<Vec<LessonVideo>, String> {
    let query = build_search_query(lesson_title, objectives);

    // Step 1: search.list — get up to 10 video candidates.
    let client = reqwest::Client::new();
    let search_resp = client
        .get("https://www.googleapis.com/youtube/v3/search")
        .query(&[
            ("part", "snippet"),
            ("type", "video"),
            ("maxResults", "10"),
            ("q", query.as_str()),
            ("key", youtube_key), // key never string-interpolated (T-11-03)
        ])
        .send()
        .await
        .map_err(|e| format!("YouTube search.list network error: {}", e))?;

    let search_status = search_resp.status().as_u16();
    let search_text = search_resp
        .text()
        .await
        .map_err(|e| format!("YouTube search.list read error: {}", e))?;

    if search_status != 200 {
        // Log status only — never the key or the full URL (T-11-03)
        log::warn!(
            "YouTube search.list returned HTTP {}; aborting video discovery",
            search_status
        );
        return Err(format!("YouTube search.list HTTP {}", search_status));
    }

    let search_json: serde_json::Value = serde_json::from_str(&search_text)
        .map_err(|e| format!("YouTube search.list JSON parse: {}", e))?;

    let mut candidates: Vec<SearchCandidate> = search_json["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let video_id = item["id"]["videoId"].as_str()?.to_string();
                    let snippet = &item["snippet"];
                    Some(SearchCandidate {
                        video_id,
                        title: snippet["title"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        channel_title: snippet["channelTitle"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        description: snippet["description"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    if candidates.is_empty() {
        return Ok(vec![]);
    }

    // Step 2: videos.list — keep only embeddable candidates (D-07).
    let video_ids: Vec<&str> = candidates.iter().map(|c| c.video_id.as_str()).collect();
    let ids_param = video_ids.join(",");

    let embed_resp = client
        .get("https://www.googleapis.com/youtube/v3/videos")
        .query(&[
            ("part", "status,snippet"),
            ("id", ids_param.as_str()),
            ("key", youtube_key), // key never string-interpolated (T-11-03)
        ])
        .send()
        .await
        .map_err(|e| format!("YouTube videos.list network error: {}", e))?;

    let embed_status = embed_resp.status().as_u16();
    let embed_text = embed_resp
        .text()
        .await
        .map_err(|e| format!("YouTube videos.list read error: {}", e))?;

    if embed_status != 200 {
        log::warn!(
            "YouTube videos.list returned HTTP {}; aborting video discovery",
            embed_status
        );
        return Err(format!("YouTube videos.list HTTP {}", embed_status));
    }

    let embed_json: serde_json::Value = serde_json::from_str(&embed_text)
        .map_err(|e| format!("YouTube videos.list JSON parse: {}", e))?;

    // Collect the set of videoIds that are embeddable.
    let embeddable_ids: std::collections::HashSet<String> = embed_json["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let id = item["id"].as_str()?.to_string();
                    let embeddable = item["status"]["embeddable"].as_bool().unwrap_or(false);
                    if embeddable { Some(id) } else { None }
                })
                .collect()
        })
        .unwrap_or_default();

    // Filter candidates to embeddable-only (D-07).
    candidates.retain(|c| embeddable_ids.contains(&c.video_id));
    log::info!(
        "video discovery: {} embeddable candidates after videos.list filter",
        candidates.len()
    );

    if candidates.is_empty() {
        return Ok(vec![]);
    }

    // Step 3: LLM ranking — score each candidate 0.0–1.0 vs lesson objective.
    let candidates_json = serde_json::json!(
        candidates.iter().map(|c| serde_json::json!({
            "videoId": c.video_id,
            "title": c.title,
            "channelTitle": c.channel_title,
            "description": &c.description[..c.description.len().min(300)],
        })).collect::<Vec<_>>()
    )
    .to_string();

    let system_prompt = format!(
        "You are an educational content curator. Given a lesson title and objectives, \
         score each video candidate for relevance on a scale of 0.0 to 1.0. \
         Return ONLY a JSON array of objects with fields: videoId (string) and \
         relevanceScore (number between 0.0 and 1.0). No explanation, no markdown.\n\n\
         Lesson title: {}\n\
         Objectives: {}",
        lesson_title,
        objectives.join(", ")
    );

    let llm_response = ai_request(
        llm_auth,
        AIServiceRequest {
            system_prompt,
            messages: vec![ServiceMessage {
                role: "user".to_string(),
                content: candidates_json,
            }],
            max_tokens: Some(1024),
            temperature: Some(0.2),
            response_format: Some("json".to_string()),
        },
    )
    .await?;

    // Step 4: Parse ranking, join to metadata, apply threshold, sort, truncate.
    let ranked: Vec<RankedVideo> = extract_json_pub(&llm_response.content)
        .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))?;

    // Build a lookup map from videoId to metadata.
    let meta_map: std::collections::HashMap<String, &SearchCandidate> =
        candidates.iter().map(|c| (c.video_id.clone(), c)).collect();

    let mut scored: Vec<LessonVideo> = ranked
        .into_iter()
        .filter(|r| r.relevance_score as f32 >= RELEVANCE_THRESHOLD)
        .filter_map(|r| {
            meta_map.get(&r.video_id).map(|c| LessonVideo {
                video_id: c.video_id.clone(),
                title: c.title.clone(),
                channel_title: c.channel_title.clone(),
                relevance_score: r.relevance_score as f32,
            })
        })
        .collect();

    // Sort descending by relevance_score.
    scored.sort_by(|a, b| {
        b.relevance_score
            .partial_cmp(&a.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Top-N only (D-02/D-08).
    scored.truncate(VIDEO_RESULT_LIMIT);

    log::info!(
        "video discovery: {} videos kept after ranking+threshold+top-N",
        scored.len()
    );

    Ok(scored)
}

// ── Discovery inner body (shared between get and refresh) ─────────────────────

/// Core discovery logic: loads module metadata, calls `fetch_and_rank_videos`,
/// persists results to `lesson_videos`, and returns them.
///
/// Called by both `get_lesson_videos` (cache miss) and `refresh_lesson_videos`
/// (after clearing rows). Returns empty list on any error (D-09).
async fn discover_and_persist(
    module_id: &str,
    youtube_key: &str,
    db_arc: &std::sync::Arc<std::sync::Mutex<crate::db::Database>>,
    auth: &AuthState,
) -> Vec<LessonVideo> {
    // Load module title + objectives inside a lock scope, drop before await.
    let (title, objectives) = {
        let db = match db_arc.lock() {
            Ok(g) => g,
            Err(e) => {
                log::warn!("video discovery: db lock error: {}", e);
                return vec![];
            }
        };
        let title: String = match db.conn.query_row(
            "SELECT title FROM modules WHERE id = ?1",
            rusqlite::params![module_id],
            |row| row.get(0),
        ) {
            Ok(t) => t,
            Err(e) => {
                log::warn!(
                    "video discovery: module {} not found: {}",
                    module_id,
                    e
                );
                return vec![];
            }
        };
        let obj_json: String = db
            .conn
            .query_row(
                "SELECT objectives_json FROM modules WHERE id = ?1",
                rusqlite::params![module_id],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "[]".to_string());
        let objectives: Vec<String> =
            serde_json::from_str(&obj_json).unwrap_or_default();
        (title, objectives)
        // db guard drops here — lock released before await
    };

    // Run YouTube + LLM discovery (no db lock held).
    let videos = match fetch_and_rank_videos(youtube_key, &title, &objectives, auth).await {
        Ok(v) => v,
        Err(e) => {
            log::warn!("video discovery error for module {}: {}", module_id, e);
            return vec![];
        }
    };

    if videos.is_empty() {
        return vec![];
    }

    // Persist results in a fresh lock scope.
    {
        let db = match db_arc.lock() {
            Ok(g) => g,
            Err(e) => {
                log::warn!("video discovery: db lock error on persist: {}", e);
                return vec![];
            }
        };
        for v in &videos {
            let id = uuid::Uuid::new_v4().to_string();
            if let Err(e) = db.conn.execute(
                "INSERT INTO lesson_videos \
                 (id, module_id, video_id, title, channel_title, relevance_score, status) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'ready')",
                rusqlite::params![
                    id,
                    module_id,
                    v.video_id,
                    v.title,
                    v.channel_title,
                    v.relevance_score,
                ],
            ) {
                log::warn!(
                    "video discovery: INSERT lesson_videos error for {}: {}",
                    v.video_id,
                    e
                );
            }
        }
        // db guard drops here
    }

    videos
}

// ── IPC commands ──────────────────────────────────────────────────────────────

/// Lazy fetch + indefinite per-lesson cache (D-03/D-04).
///
/// Cache hit: returns stored rows without any YouTube or LLM calls.
/// No key: returns empty list (D-06, silent hide).
/// Quota/offline/error: returns empty list (D-09, clean suppression).
#[tauri::command]
pub async fn get_lesson_videos(
    module_id: String,
    state: State<'_, AppState>,
    auth: State<'_, AuthState>,
) -> Result<LessonVideosResult, String> {
    let db_arc = std::sync::Arc::clone(&state.db);

    // Decision: cache hit check inside lock scope, drop before await.
    enum Decision {
        NoKey,
        FetchNeeded(String), // youtube_key
    }

    // Phase 1: check for cached ready rows — early return if present.
    {
        let db = db_arc.lock().map_err(|e| e.to_string())?;
        let cached = load_cached_videos(&db.conn, &module_id)?;
        if !cached.is_empty() {
            // Cache hit — return immediately, no API calls (D-04).
            return Ok(LessonVideosResult { videos: cached });
        }
        // db guard drops here — lock released before credential read + await.
    }

    // Phase 2: no cached rows — decide whether to fetch.
    let decision: Decision = match auth.get_credential("youtube")? {
        Some(cred) if cred.api_key.is_some() => {
            Decision::FetchNeeded(cred.api_key.unwrap())
        }
        _ => Decision::NoKey,
    };

    match decision {
        Decision::NoKey => {
            // No key configured — silently return empty (D-06).
            Ok(LessonVideosResult { videos: vec![] })
        }
        Decision::FetchNeeded(yt_key) => {
            let videos =
                discover_and_persist(&module_id, &yt_key, &db_arc, auth.inner()).await;
            Ok(LessonVideosResult { videos })
        }
    }
}

/// Manual cache invalidation + re-discovery (D-04).
///
/// Deletes all cached rows for the module, then runs fresh discovery.
/// Returns the same empty-on-error contract as `get_lesson_videos`.
#[tauri::command]
pub async fn refresh_lesson_videos(
    module_id: String,
    state: State<'_, AppState>,
    auth: State<'_, AuthState>,
) -> Result<LessonVideosResult, String> {
    let db_arc = std::sync::Arc::clone(&state.db);

    // Delete existing rows inside a lock scope, drop before await.
    let yt_key: Option<String> = {
        let db = db_arc.lock().map_err(|e| e.to_string())?;
        db.conn
            .execute(
                "DELETE FROM lesson_videos WHERE module_id = ?1",
                rusqlite::params![module_id],
            )
            .map_err(|e| e.to_string())?;
        drop(db);

        match auth.get_credential("youtube")? {
            Some(cred) => cred.api_key,
            None => None,
        }
    };

    let Some(key) = yt_key else {
        // No key — return empty (D-06).
        return Ok(LessonVideosResult { videos: vec![] });
    };

    let videos = discover_and_persist(&module_id, &key, &db_arc, auth.inner()).await;
    Ok(LessonVideosResult { videos })
}

// ── DB helpers ────────────────────────────────────────────────────────────────

/// Load cached ready rows for a module, ordered by relevance_score DESC.
fn load_cached_videos(
    conn: &rusqlite::Connection,
    module_id: &str,
) -> Result<Vec<LessonVideo>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT video_id, title, channel_title, relevance_score \
             FROM lesson_videos \
             WHERE module_id = ?1 AND status = 'ready' \
             ORDER BY relevance_score DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(rusqlite::params![module_id], |row| {
            Ok(LessonVideo {
                video_id: row.get(0)?,
                title: row.get(1)?,
                channel_title: row.get(2)?,
                relevance_score: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;

    // ── Test helpers ──────────────────────────────────────────────────────────

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn
    }

    /// Seed the full FK chain needed to insert a lesson_videos row.
    /// Returns (module_id).
    fn seed_module(conn: &Connection, title: &str, objectives_json: &str) -> String {
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Test')",
            [],
        )
        .ok();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) \
             VALUES ('tr1', 'lp1', 'Test Topic', 'devops', 'Learn stuff')",
            [],
        )
        .ok();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES ('path1', 'tr1')",
            [],
        )
        .ok();
        let mod_id = format!("mod-{}", title.replace(' ', "-").to_lowercase());
        conn.execute(
            "INSERT INTO modules (id, path_id, title, objectives_json) \
             VALUES (?1, 'path1', ?2, ?3)",
            rusqlite::params![mod_id, title, objectives_json],
        )
        .unwrap();
        mod_id
    }

    /// Insert a ready lesson_video row directly (bypass discovery).
    fn insert_video(conn: &Connection, module_id: &str, video_id: &str, score: f32) {
        conn.execute(
            "INSERT INTO lesson_videos \
             (id, module_id, video_id, title, channel_title, relevance_score, status) \
             VALUES (?1, ?2, ?3, 'Test Title', 'Test Channel', ?4, 'ready')",
            rusqlite::params![
                format!("lv-{}", video_id),
                module_id,
                video_id,
                score,
            ],
        )
        .unwrap();
    }

    // ── build_search_query tests ──────────────────────────────────────────────

    #[test]
    fn build_search_query_with_objectives_takes_first_three() {
        let objs = vec![
            "Kubernetes pods".to_string(),
            "deployments".to_string(),
            "services".to_string(),
            "ingress".to_string(), // 4th should be ignored
        ];
        let q = build_search_query("Kubernetes Basics", &objs);
        assert!(q.contains("Kubernetes Basics"));
        assert!(q.contains("Kubernetes pods"));
        assert!(q.contains("deployments"));
        assert!(q.contains("services"));
        assert!(!q.contains("ingress"), "only first 3 objectives included");
    }

    #[test]
    fn build_search_query_without_objectives_returns_title() {
        let q = build_search_query("Rust Ownership", &[]);
        assert_eq!(q, "Rust Ownership");
    }

    // ── Threshold + top-N filter tests ───────────────────────────────────────

    #[test]
    fn ranking_threshold_drops_low_score_candidates() {
        // Simulate what fetch_and_rank_videos does with the scored results.
        // Build candidates above and below the threshold.
        let candidates = vec![
            SearchCandidate {
                video_id: "v1".to_string(),
                title: "Good video".to_string(),
                channel_title: "Ch1".to_string(),
                description: "desc".to_string(),
            },
            SearchCandidate {
                video_id: "v2".to_string(),
                title: "Bad video".to_string(),
                channel_title: "Ch2".to_string(),
                description: "desc".to_string(),
            },
        ];
        let ranked_raw = vec![
            RankedVideo { video_id: "v1".to_string(), relevance_score: 0.85 },
            RankedVideo { video_id: "v2".to_string(), relevance_score: 0.4 }, // below threshold
        ];
        let meta_map: std::collections::HashMap<String, &SearchCandidate> =
            candidates.iter().map(|c| (c.video_id.clone(), c)).collect();

        let result: Vec<LessonVideo> = ranked_raw
            .into_iter()
            .filter(|r| r.relevance_score as f32 >= RELEVANCE_THRESHOLD)
            .filter_map(|r| {
                meta_map.get(&r.video_id).map(|c| LessonVideo {
                    video_id: c.video_id.clone(),
                    title: c.title.clone(),
                    channel_title: c.channel_title.clone(),
                    relevance_score: r.relevance_score as f32,
                })
            })
            .collect();

        assert_eq!(result.len(), 1, "only the video above threshold survives");
        assert_eq!(result[0].video_id, "v1");
    }

    #[test]
    fn top_n_truncation_keeps_at_most_video_result_limit() {
        // Build 5 candidates all above threshold.
        let candidates: Vec<SearchCandidate> = (1..=5)
            .map(|i| SearchCandidate {
                video_id: format!("v{}", i),
                title: format!("Video {}", i),
                channel_title: "Ch".to_string(),
                description: "desc".to_string(),
            })
            .collect();
        let ranked_raw: Vec<RankedVideo> = (1..=5)
            .map(|i| RankedVideo {
                video_id: format!("v{}", i),
                relevance_score: 0.7,
            })
            .collect();
        let meta_map: std::collections::HashMap<String, &SearchCandidate> =
            candidates.iter().map(|c| (c.video_id.clone(), c)).collect();

        let mut result: Vec<LessonVideo> = ranked_raw
            .into_iter()
            .filter(|r| r.relevance_score as f32 >= RELEVANCE_THRESHOLD)
            .filter_map(|r| {
                meta_map.get(&r.video_id).map(|c| LessonVideo {
                    video_id: c.video_id.clone(),
                    title: c.title.clone(),
                    channel_title: c.channel_title.clone(),
                    relevance_score: r.relevance_score as f32,
                })
            })
            .collect();
        result.truncate(VIDEO_RESULT_LIMIT);

        assert_eq!(
            result.len(),
            VIDEO_RESULT_LIMIT,
            "top-N truncates to VIDEO_RESULT_LIMIT={}",
            VIDEO_RESULT_LIMIT
        );
    }

    #[test]
    fn embeddable_filter_drops_non_embeddable_candidates() {
        // Simulate the embeddable filtering step.
        let embeddable_ids: std::collections::HashSet<String> =
            vec!["v1".to_string()].into_iter().collect();
        let mut candidates = vec![
            SearchCandidate {
                video_id: "v1".to_string(),
                title: "Embeddable".to_string(),
                channel_title: "Ch".to_string(),
                description: "desc".to_string(),
            },
            SearchCandidate {
                video_id: "v2".to_string(),
                title: "Not embeddable".to_string(),
                channel_title: "Ch".to_string(),
                description: "desc".to_string(),
            },
        ];
        candidates.retain(|c| embeddable_ids.contains(&c.video_id));

        assert_eq!(candidates.len(), 1, "non-embeddable candidates are dropped (D-07)");
        assert_eq!(candidates[0].video_id, "v1");
    }

    // ── load_cached_videos tests ──────────────────────────────────────────────

    #[test]
    fn load_cached_videos_returns_ready_rows_ordered_by_score() {
        let conn = fresh_conn();
        let mod_id = seed_module(&conn, "Kubernetes Pods", "[]");

        insert_video(&conn, &mod_id, "abc", 0.75);
        insert_video(&conn, &mod_id, "xyz", 0.92);
        insert_video(&conn, &mod_id, "def", 0.60);

        let videos = load_cached_videos(&conn, &mod_id).unwrap();
        assert_eq!(videos.len(), 3);
        // Ordered by relevance_score DESC
        assert_eq!(videos[0].video_id, "xyz"); // 0.92
        assert_eq!(videos[1].video_id, "abc"); // 0.75
        assert_eq!(videos[2].video_id, "def"); // 0.60
    }

    #[test]
    fn load_cached_videos_returns_empty_when_no_rows() {
        let conn = fresh_conn();
        let mod_id = seed_module(&conn, "Empty Module", "[]");
        let videos = load_cached_videos(&conn, &mod_id).unwrap();
        assert!(videos.is_empty());
    }

    #[test]
    fn load_cached_videos_ignores_non_ready_rows() {
        let conn = fresh_conn();
        let mod_id = seed_module(&conn, "Module With Pending", "[]");

        // Insert a 'pending' status row — should be filtered out.
        conn.execute(
            "INSERT INTO lesson_videos \
             (id, module_id, video_id, title, channel_title, relevance_score, status) \
             VALUES ('lv-pending', ?1, 'v-pending', 'Pending', 'Ch', 0.9, 'pending')",
            rusqlite::params![mod_id],
        )
        .unwrap();

        let videos = load_cached_videos(&conn, &mod_id).unwrap();
        assert!(
            videos.is_empty(),
            "pending rows must not be returned by load_cached_videos"
        );
    }

    // ── camelCase serde contract tests ────────────────────────────────────────

    #[test]
    fn lesson_video_serializes_with_camel_case() {
        let v = LessonVideo {
            video_id: "abc123".to_string(),
            title: "Test".to_string(),
            channel_title: "Ch".to_string(),
            relevance_score: 0.8,
        };
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("\"videoId\""), "must serialize as videoId (camelCase)");
        assert!(json.contains("\"channelTitle\""), "must serialize as channelTitle (camelCase)");
        assert!(json.contains("\"relevanceScore\""), "must serialize as relevanceScore (camelCase)");
    }

    #[test]
    fn lesson_videos_result_serializes_with_camel_case() {
        let r = LessonVideosResult {
            videos: vec![LessonVideo {
                video_id: "v1".to_string(),
                title: "T".to_string(),
                channel_title: "C".to_string(),
                relevance_score: 0.7,
            }],
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"videos\""), "result must have 'videos' key");
    }

    // ── Constants presence tests ──────────────────────────────────────────────

    #[test]
    fn constants_have_expected_values() {
        assert_eq!(VIDEO_RESULT_LIMIT, 3, "VIDEO_RESULT_LIMIT must be 3 (D-02)");
        assert!(
            RELEVANCE_THRESHOLD >= 0.5 && RELEVANCE_THRESHOLD <= 0.8,
            "RELEVANCE_THRESHOLD {} should be in [0.5, 0.8]",
            RELEVANCE_THRESHOLD
        );
    }
}
