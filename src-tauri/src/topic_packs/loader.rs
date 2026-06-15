//! Pack loader — bundled (compile-time) + skills (runtime) discovery.
//!
//! ## Wave 0 status
//!
//! Every `pub fn` here returns either `Err(PackError::Loader(...))` or
//! `unimplemented!()` with a message naming Wave 1 (Plan 05-02). The
//! [`BUNDLED_PACKS`] static IS already wired — `include_dir!` walks the
//! `topic-packs/` tree at compile time so Wave 1 just iterates it.
//!
//! All eight `#[cfg(test)] mod tests` cases are RED with failure messages
//! that name the wave/plan turning them GREEN.

use include_dir::{include_dir, Dir};
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::error::PackError;
use super::model::LoadedPack;

/// Compile-time-embedded `topic-packs/` directory.
///
/// Path is relative to `$CARGO_MANIFEST_DIR` (= `src-tauri/`). Resolves to
/// `<repo>/topic-packs/`, picking up every pack directory automatically —
/// no per-pack code edit needed.
pub static BUNDLED_PACKS: Dir<'_> =
    include_dir!("$CARGO_MANIFEST_DIR/../topic-packs");

/// In-memory registry of loaded packs keyed by `pack.id`.
#[derive(Debug, Default)]
pub struct PackRegistry {
    pub packs: BTreeMap<String, LoadedPack>,
}

/// Resolve the user-skills directory (`~/.learnforge/skills/`).
///
/// Returns `None` if the home directory cannot be determined. Wave 1 will
/// also create the directory on first call to ease onboarding (D-03).
pub fn skills_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".learnforge").join("skills"))
}

/// Load all bundled + skill packs into a fresh registry.
///
/// Wave 0: returns Err — Wave 1 (Plan 05-02) implements the body. The
/// error message intentionally names the downstream plan so a developer
/// running the test suite sees the correct next step.
pub fn load_all() -> Result<PackRegistry, PackError> {
    Err(PackError::Loader(
        "Wave 1 (Plan 05-02) must implement load_all — see plan 05-02 Task 1".to_string(),
    ))
}

/// Re-load just the skill packs into an existing registry, preserving the
/// bundled set (D-03 bundled-wins-on-collision).
///
/// Wave 0: returns Err. Wave 1 wires the body.
pub fn reload_skills_into(_registry: &mut PackRegistry) -> Result<(), PackError> {
    Err(PackError::Loader(
        "Wave 1 (Plan 05-02) must implement reload_skills_into — see plan 05-02 Task 1"
            .to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RED — Wave 1 (Plan 05-02) must turn this GREEN.
    ///
    /// `BUNDLED_PACKS.dirs().count()` must surface all 6 pack directories
    /// (4 existing + 2 new from Plan 05-01) AND `load_all()` must return
    /// `Ok` with 6 entries. Today the count IS 6 (include_dir is wired),
    /// but `load_all()` returns `Err`, so this fails on the load assertion.
    #[test]
    fn bundled_loads_all() {
        let dir_count = BUNDLED_PACKS.dirs().count();
        assert!(
            dir_count >= 6,
            "Wave 1 (Plan 05-02) must turn this GREEN: BUNDLED_PACKS must contain at least 6 pack dirs (got {})",
            dir_count
        );
        let registry = load_all().expect(
            "Wave 1 (Plan 05-02) must implement load_all — bundled_loads_all expects 6 packs",
        );
        assert_eq!(
            registry.packs.len(),
            6,
            "Wave 1 (Plan 05-02) must turn this GREEN: load_all must return 6 packs (4 existing + agentic-devops + ai-engineering); got {}",
            registry.packs.len()
        );
    }

    /// RED — Wave 1 (Plan 05-02) must turn this GREEN.
    ///
    /// Drops a fake `pack.json` into a TempDir-backed skills path and asserts
    /// the loader picks it up with `source = Skill`. Wave 1 introduces a
    /// `load_all_from` helper accepting a custom skills root so the test can
    /// avoid touching the real `~/.learnforge/skills/`.
    #[test]
    fn skills_picked_up() {
        let _ = load_all().expect(
            "Wave 1 (Plan 05-02) must turn this GREEN: skills_picked_up needs a load_all_from(skills_root) helper that accepts a temp dir",
        );
    }

    /// RED — Wave 1 (Plan 05-02) must turn this GREEN.
    ///
    /// Writes a skill with id `kubernetes-fundamentals` (collides with a
    /// bundled pack) and asserts the skill is DROPPED and a warning is
    /// surfaced. D-03 says bundled wins.
    #[test]
    fn collision_bundled_wins() {
        let _ = load_all().expect(
            "Wave 1 (Plan 05-02) must turn this GREEN: collision_bundled_wins needs load_all_from + skill-collision detection (D-03)",
        );
    }

    /// RED — Wave 1 (Plan 05-02) must turn this GREEN once it iterates the
    /// 6 bundled packs and validates each against the schema.
    ///
    /// The 4 existing packs CURRENTLY lack `pack_version`; that's optional
    /// per the schema so it should not block validation. Wave 5 (Plan 05-06)
    /// is the format-upgrade pass that adds the field everywhere.
    #[test]
    fn existing_packs_valid() {
        let registry = load_all().expect(
            "Wave 1 (Plan 05-02) must turn this GREEN: existing_packs_valid needs load_all to iterate BUNDLED_PACKS and run the schema validator on each pack.json",
        );
        for (id, lp) in &registry.packs {
            assert_eq!(
                lp.validation_status,
                super::super::model::ValidationStatus::Ok,
                "{} must validate as Ok — Wave 5 (Plan 05-06) format-upgrades the 4 existing packs if this trips on missing-required fields",
                id
            );
        }
    }

    /// RED — Wave 1 must surface `ValidationStatus::Errors` on a pack
    /// missing the `id` field. Plan 05-02 wires the strict/soft classifier
    /// per D-07.
    #[test]
    fn strict_rejects_required() {
        let _ = load_all().expect(
            "Wave 1 (Plan 05-02) must turn this GREEN: strict_rejects_required needs load_all_from + strict-classifier (D-07)",
        );
    }

    /// RED — Wave 1 must surface `ValidationStatus::Warnings` on a pack
    /// missing only optional fields (`estimated_hours`, etc.). D-07 soft-warn.
    #[test]
    fn soft_warns_optional() {
        let _ = load_all().expect(
            "Wave 1 (Plan 05-02) must turn this GREEN: soft_warns_optional needs load_all_from + soft-warn classifier (D-07)",
        );
    }
}
