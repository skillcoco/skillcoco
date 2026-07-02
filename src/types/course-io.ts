/**
 * Phase 12 — Course Import/Export
 *
 * TypeScript types for the export_course and import_course Tauri commands.
 * Field names are camelCase matching the Rust `#[serde(rename_all = "camelCase")]`
 * wire format defined in src-tauri/src/commands/course_io.rs (Plans 02/03).
 *
 * These types cross the renderer → Tauri command boundary defined in the
 * threat model (T-12-13, T-12-14, T-12-17).
 */

// ── Export ──

/** Request envelope for the export_course Tauri command. */
export interface ExportCourseRequest {
  /** The learning track to export. */
  trackId: string;
  /** Absolute path chosen by the user via the native save dialog. */
  savePath: string;
}

/** Successful result from export_course. */
export interface ExportCourseResult {
  /** The path the file was actually written to. */
  savedPath: string;
  /** Number of ready blocks included in the export. */
  blockCount: number;
  /** Number of modules included in the export. */
  moduleCount: number;
}

// ── Import ──

/** Request envelope for the import_course Tauri command. */
export interface ImportCourseRequest {
  /** Absolute path to the .json course file chosen via the native open dialog. */
  filePath: string;
}

/** Successful result from import_course. */
export interface ImportCourseResult {
  /** The newly-created track ID (namespaced, distinct per import — D-09). */
  trackId: string;
  /** Number of modules rehydrated from the file. */
  moduleCount: number;
  /** Number of ready blocks rehydrated from the file. */
  blockCount: number;
  /** Non-fatal warnings surfaced during import (e.g. skipped non-ready blocks). */
  warnings: string[];
}
