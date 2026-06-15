fn main() {
    // CR-01 (Phase 5 review): `include_dir!()` does NOT emit cargo
    // `rerun-if-changed` directives for its embedded tree. Without these
    // hints, edits to `topic-packs/*/pack.json` silently use stale
    // compile-time bytes until `cargo clean`. Emit one directive for the
    // tree root (cargo stats the directory mtime, which catches
    // added/deleted files) and one for the schema file (whose changes are
    // pure-content edits that bump the file mtime but not the directory's).
    println!("cargo:rerun-if-changed=../topic-packs");
    println!("cargo:rerun-if-changed=../topic-packs/pack-schema.json");
    tauri_build::build()
}
