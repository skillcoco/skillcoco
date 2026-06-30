/**
 * Phase 11 — Video-Enriched Lessons
 *
 * Types for the per-lesson video cache (lesson_videos table, migration v012).
 * Field names are camelCase to match the Rust `#[serde(rename_all = "camelCase")]`
 * wire format used by the backend IPC structs in src-tauri/src/commands/videos.rs
 * (defined in Plan 02).
 *
 * Video content is a lesson-level adjunct panel — see the NOTE in
 * src/types/learning.ts near BlockType for why "video" is not a BlockType variant.
 */

/** A single cached YouTube video associated with a lesson module. */
export interface LessonVideo {
  videoId: string;
  title: string;
  channelTitle: string;
  relevanceScore: number;
}

/** IPC result type returned by the get_lesson_videos and refresh_lesson_videos commands. */
export interface LessonVideosResult {
  videos: LessonVideo[];
}
