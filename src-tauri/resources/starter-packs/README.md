# Bundled Starter Packs

Curated free course-export JSON files bundled with the app (D-12). These ship
fully offline — no Hub/server catalog fetch is involved in listing or starting
them.

## What lives here

Each `*.json` file is a full `CourseExportPayload` (the same shape produced by
`export_course_impl` / consumed by `import_course_impl` in
`src-tauri/src/commands/course_io.rs`), not an outline-only topic pack. This
lets a starter pack "Start" action route through the exact same fail-closed
import gate as a user-picked file import (D-13) — there is no special-case
bypass command for bundled content.

`exportedFrom` on every file uses the `topic-pack:<id>` provenance convention
(the existing bundled/authored-content class from
`is_course_exportable`/`provenance_tier` in `course_io.rs`). None of these
files use a `licensed:`/`curated:` prefix — starter packs are always free,
non-reserved provenance.

## Refresh cadence

Refreshed on app releases. There is no runtime update mechanism in this
phase — replacing the bundled JSON files and shipping a new app version is
the only way to update starter-pack content.

## How starting a pack works

`list_starter_packs` enumerates the `*.json` files in this directory (via
Tauri's `resource_dir()`) and returns lightweight metadata for Library tiles.
`start_starter_pack` resolves a chosen pack id to its file inside this
directory (with a path-traversal guard) and calls the UNCHANGED
`import_course_impl(conn, resolved_path)` — the identical gate a normal
file-picker import uses.
