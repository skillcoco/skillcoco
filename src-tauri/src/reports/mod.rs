//! Phase 18 (Plan 04) — signed skill report PDF rendering.
//!
//! `artifacts` holds the report PDF renderer (`ReportPdfInput` +
//! `render_report_pdf`), sibling to `achievements::artifacts`'
//! certificate/badge renderers. `commands/reports.rs` is a *separate*
//! module (the 18-03 IPC layer) — this `reports` module is src-tauri-side
//! rendering only; the pure assembly algorithm lives in
//! `learnforge_core::reports`.

pub mod artifacts;
