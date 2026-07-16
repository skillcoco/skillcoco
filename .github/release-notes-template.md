<!--
SkillCoco release-notes template — per locked O-7 (curated CHANGELOG
extract, NOT raw auto-generated PR list).

Usage:
1. tauri-action creates a draft GitHub Release with auto-generated PR-list
   notes. Replace that body with a curated version using this template.
2. Extract the matching version section from CHANGELOG.md (top-level for
   desktop releases; skillcoco-core/CHANGELOG.md for crate releases) and
   paste under "## What changed."
3. Update the install instructions if a platform's artifact name changed.
4. Update the source / crate URLs to point at the new tag.

For the bootstrap v0.1.0 release see docs/LAUNCH-PLAYBOOK.md step 6.
-->

# SkillCoco desktop {VERSION} — {ONE_LINE_SUMMARY}

Released {DATE}. Open source under MIT (algorithms and whitepapers
under CC BY 4.0). API stability: pre-1.0 — see
[VERSIONING.md](https://github.com/skillcoco/skillcoco/blob/main/docs/VERSIONING.md).

## What changed

<!-- Paste the matching CHANGELOG section here. Trim trailing
comparison-URL footnotes. Preserve Keep-a-Changelog subsection order:
Added → Changed → Deprecated → Removed → Fixed → Security. -->

### Added

- ...

### Changed

- ...

### Fixed

- ...

### Security

- ...

## Install

- **macOS (notarized):** download the `.dmg` matching your architecture
  (`aarch64` for Apple Silicon, `x64` for Intel), open the DMG, drag
  `SkillCoco.app` to Applications. If macOS asks "Are you sure" on
  first launch, click "Open." Notarization is stapled, so no Gatekeeper
  bypass is required.
- **Linux (AppImage):** download `skillcoco_{VERSION}_amd64.AppImage`,
  `chmod +x skillcoco_{VERSION}_amd64.AppImage`, run it.
- **Linux (Debian/Ubuntu):** download `skillcoco_{VERSION}_amd64.deb`,
  `sudo dpkg -i skillcoco_{VERSION}_amd64.deb`.
- **Windows (installer):** download `SkillCoco_{VERSION}_x64-setup.exe`,
  double-click, follow the wizard.
- **Windows (MSI):** download `SkillCoco_{VERSION}_x64_en-US.msi`,
  right-click → Install.

Windows and Linux builds are UNSIGNED per D-02b — SmartScreen will warn
on first launch; click "More info" → "Run anyway." If you would prefer
signed Windows binaries, please open a discussion in
[Ideas](https://github.com/skillcoco/skillcoco/discussions/categories/ideas);
we are tracking install metrics before deciding whether to invest in
an EV code-signing certificate.

## Source

- **Tag:** https://github.com/skillcoco/skillcoco/tree/{TAG}
- **Crate (`skillcoco-core`):** https://crates.io/crates/skillcoco-core/{CRATE_VERSION}
- **Docs:** https://docs.rs/skillcoco-core/{CRATE_VERSION}
- **Per-crate changelog:** https://github.com/skillcoco/skillcoco/blob/{TAG}/skillcoco-core/CHANGELOG.md

## Verify

- **macOS:** `spctl --assess --type execute --verbose=4 /Applications/SkillCoco.app`
  → expected output `accepted, source=Notarized Developer ID`.
- **Crate:** `cargo search skillcoco-core` → expected
  `skillcoco-core = "{CRATE_VERSION}"`.

## Feedback

- **Bug reports:** [Issues](https://github.com/skillcoco/skillcoco/issues)
- **Usage questions:** [Q&A discussions](https://github.com/skillcoco/skillcoco/discussions/categories/q-a)
- **Feature requests:** [Ideas discussions](https://github.com/skillcoco/skillcoco/discussions/categories/ideas)
- **Show off your skill packs:** [Show & Tell](https://github.com/skillcoco/skillcoco/discussions/categories/show-and-tell)
- **Security vulnerabilities:** see [SECURITY.md](https://github.com/skillcoco/skillcoco/blob/main/SECURITY.md);
  please use the "Report a vulnerability" button on the
  [Security](https://github.com/skillcoco/skillcoco/security) tab.

---

*Slow-burn launch per [D-04](../.planning/phases/08-publishing/08-CONTEXT.md).
No HN / Reddit / Lobsters coordination — wider launch deferred to v1.0
(~Dec 2026). If you found this release organically, thank you for the
trust; we are not asking you to amplify it.*
