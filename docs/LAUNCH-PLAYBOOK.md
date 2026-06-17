# LearnForge Launch Playbook

Operational instructions for the maintainer to execute Phase 8 Wave 7's
human-action checkpoints. This document is the **how**; the
[`08-ACCEPTANCE.md`](../.planning/phases/08-publishing/08-ACCEPTANCE.md)
walkthrough is the **what + sign-off**.

> This playbook is the canonical reference for every future LearnForge
> release. It covers the bootstrap (one-time setup + first publish) plus
> the steady-state flow (subsequent 0.1.1+ patches and 0.2.0+ minors).

---

## Pre-launch checklist (5 minutes, before any of the below)

Run these on the maintainer's machine before starting any human-gated
checkpoint:

```bash
cd /Users/gshah/gsd-workspaces/studio-split   # or your checkout path
git remote -v                                  # confirm origin = agentixgarage/learnforge
git status                                     # working tree clean
git fetch origin && git log --oneline origin/main..HEAD  # confirm branch is current
cargo --version                                # 1.75+ recommended
gh --version                                   # any recent version
```

If anything is unexpected, stop and resolve before proceeding. The
checkpoints below assume a clean checkout on a branch with a sensible
HEAD.

---

## crates.io bootstrap publish

**Locked decisions:** D-01, D-01b, O-5. **STRIDE refs:** T-08-LAUNCH-01
(slopsquat mitigation), T-08-LAUNCH-02 (token-rotation mitigation).

Per RESEARCH Pitfall 3, Trusted Publishing requires an existing crate
on crates.io — but the crate does not exist until the first publish.
The first publish MUST therefore be manual from the maintainer's
authenticated machine.

### Step 1 — Authenticate to crates.io

```bash
# Visit https://crates.io/me and generate a personal API token.
# Token scope: "publish-new" + "publish-update" minimum.
# Copy it to clipboard.
cargo login
# Paste the token at the prompt; the value lands in ~/.cargo/credentials.toml
# (mode 600). Do not commit this file.
```

### Step 2 — Pre-flight regression gates

Every one of these MUST exit 0 before the real publish runs. Do not
trust an old dry-run from Phase 7 — re-run today.

```bash
cargo build -p learnforge-core
cargo test  -p learnforge-core
cargo publish --dry-run -p learnforge-core

# Repo-URL drift check (RESEARCH Pitfall 6 / Wave 1 R1 fix)
rg "schoolofdevops/learnforge" learnforge-core/    # MUST return 0 lines

# docs.rs metadata block exists (Wave 1 R7 fix)
grep "\[package.metadata.docs.rs\]" learnforge-core/Cargo.toml  # MUST return 1 line

# Optional: clean stale package dir from Phase 7 dry-runs
cargo clean -p learnforge-core
```

### Step 3 — The real publish

```bash
cargo publish -p learnforge-core
# Watch for "uploaded learnforge-core v0.1.0 to crates.io" and HTTP 200.
# If you see HTTP 4xx, READ the error carefully and abort — do not
# retry blindly. Common rejections at this stage: name collision (someone
# else owns "learnforge-core"), missing required metadata, or yanked
# version conflict (impossible for first publish but check).
```

### Step 4 — Verify live (60 second indexing delay)

```bash
sleep 60
cargo search learnforge-core | head -3
# Expected: learnforge-core = "0.1.0"    # Adaptive learning algorithms ...

# Sanity-check the rendered page
open https://crates.io/crates/learnforge-core
# Verify: README renders, repository link goes to agentixgarage/learnforge,
# docs.rs badge resolves (within 1 hour after publish — docs.rs builds
# can lag).
```

**Brief success criterion #13 satisfied here.**

---

## Trusted Publishing post-publish setup

**Locked decisions:** D-01b, O-5. **STRIDE refs:** T-08-LAUNCH-02
(static token rotation).

Now that the crate exists on crates.io, Trusted Publishing can be
configured. After this is done, every future tag-triggered publish
(0.1.1, 0.2.0, ...) uses short-lived OIDC tokens instead of a static
`CARGO_REGISTRY_TOKEN`.

### Step 1 — Add the org as co-owner

```bash
# Per locked D-01b: agentixgarage GitHub org is the long-term crate owner;
# the personal account is just the bootstrap account.
cargo owner --add github:agentixgarage:learnforge-maintainers -p learnforge-core
cargo owner --list -p learnforge-core
# Expected: both your personal account AND the agentixgarage team appear.
```

> If `learnforge-maintainers` team does not exist, create it first at
> https://github.com/orgs/agentixgarage/new-team. Members of that team
> can act as crate co-owners.

### Step 2 — Configure Trusted Publishing in the crates.io UI

Visit https://crates.io/crates/learnforge-core/settings → Trusted
Publishers section → "Add new publisher."

Fill in EXACTLY these values (no typos — the OIDC claim must match):

| Field | Value |
|-------|-------|
| Repository owner | `agentixgarage` |
| Repository name | `learnforge` |
| Workflow filename | `core-publish.yml` |
| Environment name | `release` |

Click **Save**. The publisher row should now appear in the Trusted
Publishers list.

> `core-publish.yml` references this `release` environment in its YAML
> (`environment: release` at the job level). The matching is bidirectional
> — the workflow asks for env "release," and crates.io trusts requests
> from env "release."

### Step 3 — Revoke the personal API token (T-08-LAUNCH-02 mitigation)

The personal token used in the bootstrap publish must not persist. With
Trusted Publishing live, it is now an attack surface (long-lived static
token in `~/.cargo/credentials.toml`).

```bash
# 1. List tokens at https://crates.io/settings/tokens
# 2. Revoke the token used for the bootstrap publish.
# 3. Remove it from local credentials store. `cargo logout` is the
#    canonical approach (cargo 1.70+) — it removes the
#    [registry.crates-io] block plus its token = "..." line from
#    ~/.cargo/credentials.toml. (Earlier playbook revisions paired
#    this with a sed fallback that did NOT match the actual TOML
#    format and would have left the token in place — removed.)
cargo logout

# 4. Verify the token is gone:
grep -A2 '^\[registry\.crates-io\]' ~/.cargo/credentials.toml || \
  echo "OK: [registry.crates-io] block removed"
```

**After this step, manual `cargo publish` will fail with auth errors —
which is correct.** Future publishes flow through `core-publish.yml`.

---

## First desktop release

**Locked decisions:** D-06, D-06b, D-06c, O-7, O-8. **STRIDE refs:**
T-08-LAUNCH-03 (tag-order spoof mitigation), T-08-LAUNCH-05
(notarization receipt).

Per locked O-8 tag ordering, the crate must be live on crates.io
BEFORE the desktop tag is pushed. Verify this every single time.

### Step 1 — Final regression + tag-order gate

```bash
# CONFIRM crates.io publish is live (locked O-8 gate)
cargo search learnforge-core | head -1
# Expected: "learnforge-core = "0.1.0""
# If empty or shows older version, STOP — do not push the desktop tag.

# Workspace-wide build + test (the desktop tag triggers a 3-OS matrix
# build via release.yml; this local gate catches regressions before
# committing 3 OS-runner minutes per platform per attempt).
cargo build --workspace
cargo test  --workspace

# Final crate gate
cargo publish --dry-run -p learnforge-core    # MUST exit 0
```

### Step 2 — Push the crate marker tag (`core-v0.1.0`)

```bash
git tag -a core-v0.1.0 -m "learnforge-core 0.1.0 — first crates.io publish (Phase 8 Wave 7)"
git push origin core-v0.1.0
```

This tag triggers `core-publish.yml`, which will run end-to-end and
fail at `cargo publish` with HTTP 400 "crate version already uploaded."
**This failure is EXPECTED and acceptable.** It is the OIDC flow's
first authenticated run; the workflow runs in full to validate the
Trusted Publishing config. Subsequent `core-v0.1.1` tags will succeed
because the version will be new.

Verify in https://github.com/agentixgarage/learnforge/actions: the
`core-publish.yml` run failed at the publish step, NOT before. If it
failed earlier (e.g., at the OIDC token exchange), Trusted Publishing
is misconfigured — re-check Step 2 of the previous section.

### Step 3 — Push the desktop release tag (`v0.1.0`)

```bash
git tag -a v0.1.0 -m "LearnForge desktop v0.1.0 — first GitHub Release (Phase 8 Wave 7)"
git push origin v0.1.0
```

### Step 4 — Watch release.yml

Open https://github.com/agentixgarage/learnforge/actions and pick the
newly triggered "Release Desktop" run.

The matrix has 4 entries:

| Entry | Expected duration | Common failure modes |
|-------|-------------------|----------------------|
| `macos-latest --target aarch64-apple-darwin` | 5-30 min (notarization-dominated) | Notarization stuck; entitlements mismatch (RESEARCH Pitfall 2) |
| `macos-latest --target x86_64-apple-darwin` | 5-30 min | Same as arm64 |
| `ubuntu-latest` | 3-7 min | apt missing packages (libwebkit2gtk-4.1-dev etc.) |
| `windows-latest` | 5-10 min | Rare; mostly works first-shot |

If C1 (Apple Developer enrollment) was deferred per the D-02b fallback,
the macOS builds still produce an unsigned `.dmg` — the matrix entries
go green but the artifacts carry no notarization receipt. Install docs
explain the Gatekeeper bypass for users.

If a matrix entry fails:

- **Read the log** before retrying. Most failures are environmental
  (apt mirror flake, GitHub Actions runner image change) and recoverable
  with a retry.
- **If notarization is stuck > 30 minutes**, do NOT retry — that
  re-submits and doubles the queue position. Wait 1 hour, then check
  the workflow logs for the Apple submission UUID, run
  `xcrun notarytool log <UUID> --apple-id ... --team-id ... --password ...`
  on your local machine to surface Apple's actual error.
- **If a permanent regression** (entitlements mismatch, signing
  identity wrong), fix forward in a patch: rollback the tag with
  `git tag -d v0.1.0 && git push origin :refs/tags/v0.1.0`, fix the
  bug, re-push the tag.

### Step 5 — Review the DRAFT GitHub Release

Open https://github.com/agentixgarage/learnforge/releases. The matrix's
final artifacts have been uploaded to a draft v0.1.0 release.

Verify 6+ artifacts are present (brief criterion #14):

- `LearnForge_0.1.0_aarch64.dmg` (macOS arm64)
- `LearnForge_0.1.0_x64.dmg` (macOS x86_64)
- `learnforge_0.1.0_amd64.AppImage` (Linux AppImage)
- `learnforge_0.1.0_amd64.deb` (Linux Debian package)
- `LearnForge_0.1.0_x64-setup.exe` (Windows installer)
- `LearnForge_0.1.0_x64_en-US.msi` (Windows MSI)

(Exact filenames depend on tauri-action default; pattern is
`{productName}_{version}_{arch}.{ext}`.)

### Step 6 — Curate release notes (locked O-7)

`tauri-action` produces auto-generated release notes from the merged-PR
list since the prior tag. For the FIRST release, that list is the
entire history of the project, which is noisy.

Replace it with a curated extract from the top-level `CHANGELOG.md`:

```bash
# Extract the v0.1.0 section
awk '/^## \[0\.1\.0\]/,/^## \[/' CHANGELOG.md | sed '$d' > /tmp/release-notes-v0.1.0.md
# Append a footer linking the per-crate CHANGELOG
cat <<'EOF' >> /tmp/release-notes-v0.1.0.md

---

**Install:**

- macOS (notarized): download the `.dmg` for your architecture, mount,
  drag LearnForge.app to Applications.
- Linux: `.AppImage` (chmod +x and run) or `.deb` (dpkg -i).
- Windows: `.exe` installer or `.msi` (right-click → Install).

**Source:** https://github.com/agentixgarage/learnforge/tree/v0.1.0
**Crate:** https://crates.io/crates/learnforge-core/0.1.0
**Per-crate changelog:** [learnforge-core/CHANGELOG.md](https://github.com/agentixgarage/learnforge/blob/v0.1.0/learnforge-core/CHANGELOG.md)
EOF
```

Open the draft release on GitHub, click "Edit," paste the curated notes
into the release-notes body, replacing the auto-generated content.

### Step 7 — Publish the release

Click "Publish release." The release becomes public; the v0.1.0 tag is
no longer a draft.

**Brief success criterion #14 satisfied here.**

### Step 8 — Smoke-test macOS notarization (if C1 approved)

```bash
# Download the macOS .dmg matching your machine
spctl --assess --type execute --verbose=4 /Volumes/LearnForge/LearnForge.app
# Expected: "accepted, source=Notarized Developer ID"
```

Any other output (`rejected`, `source=Unknown`, `source=Developer ID`
without `Notarized`) means notarization didn't staple correctly — open
a Phase 8.1 follow-up issue.

---

## GitHub Discussions + Private Vulnerability Reporting

**Locked decisions:** D-07, D-07b. **STRIDE refs:** none (community
surface; not on a trust boundary).

### Step 1 — Enable Discussions

1. Visit https://github.com/agentixgarage/learnforge/settings
2. Features section → check "Discussions"
3. Reload the page.

### Step 2 — Create the 4 category slugs

Visit https://github.com/agentixgarage/learnforge/discussions → gear
icon → "Manage categories."

Create (delete the auto-created defaults if needed):

| Category name | Required slug | Discussion format |
|---------------|---------------|---------------------|
| Q&A | `q-a` | Question / Answers |
| Ideas | `ideas` | Open-ended discussion |
| Show & Tell | `show-and-tell` | Open-ended discussion |
| General | `general` | Open-ended discussion |

> The slug auto-generates from the name. "Q&A" becomes `q-a` because
> ampersands are stripped and the result is lowercased. To verify the
> slug, click the category and check the URL path:
> `/discussions/categories/<slug>`.

If the slug does not match the filename in
`.github/DISCUSSION_TEMPLATE/`, the template will not render — per
RESEARCH Pitfall 4. Adjust the category name until the slug matches.

### Step 3 — Verify each template renders

For each category:

1. Click "New discussion"
2. Select the category
3. Confirm the form fields from `.github/DISCUSSION_TEMPLATE/<slug>.yml`
   appear (e.g., for Q&A: platform dropdown, version field, "What have
   you tried?" textarea)

If a form does not render, the slug-to-filename mapping is broken —
re-check Step 2.

### Step 4 — Enable Private Vulnerability Reporting

1. Visit https://github.com/agentixgarage/learnforge/settings/security_analysis
2. Find "Private vulnerability reporting" → click "Enable"
3. Verify: visit https://github.com/agentixgarage/learnforge/security —
   "Report a vulnerability" button is visible

This wires the SECURITY.md vulnerability-reporting policy (Wave 1) to
GitHub's structured intake. Researchers no longer have to email the
fallback address; they can use the structured form.

---

## Private outreach (5-10 individuals)

See [`08-07-OUTREACH.md`](../.planning/phases/08-publishing/08-07-OUTREACH.md)
for the template, distribution-list categories, and do's-and-don'ts.

The playbook for this step is: open the OUTREACH.md template, pick
5-10 individuals, customize the specific-ask paragraph per recipient,
send via the recipient's preferred channel, track outcomes in private
notes outside the repo.

**The recipient list is NEVER committed to the repo.** Verify with
`git log --all --oneline` after Phase 8 closes — no commit messages or
file contents should reference recipient names.

---

## Steady-state release flow (Phase 8.1+)

After Phase 8 ships, future releases follow this much simpler flow:

### Patch release (0.1.x → 0.1.x+1)

```bash
# 1. Cut a release branch (or work on main)
git checkout -b release/0.1.1
# 2. Bump versions
# Edit learnforge-core/Cargo.toml: version = "0.1.1"
# Edit src-tauri/Cargo.toml: version = "0.1.1"  (track per D-03c)
# Edit src-tauri/tauri.conf.json: version "0.1.1"
# 3. Update CHANGELOGs (both top-level and per-crate)
# Move [Unreleased] → [0.1.1] - YYYY-MM-DD; add new empty [Unreleased]
# 4. Commit + PR + merge
# 5. Push BOTH tags (locked O-8 ordering)
git checkout main && git pull
git tag -a core-v0.1.1 -m "learnforge-core 0.1.1"
git push origin core-v0.1.1
# Wait for crates.io to confirm via cargo search
sleep 60 && cargo search learnforge-core | head -1
# Then desktop tag
git tag -a v0.1.1 -m "LearnForge desktop v0.1.1"
git push origin v0.1.1
# 6. Watch release.yml; review draft; publish release
```

This is fully automated end-to-end — no manual `cargo publish`, no
keychain, no token shuffling.

### Minor release (0.X.0 → 0.X+1.0)

Same as patch, except:

- May contain breaking changes per D-08c (allowed pre-1.0 per semver)
- CHANGELOG `### Changed` and `### Removed` sections should explicitly
  list breaking changes; consumers reading the changelog should be
  able to anticipate migration work
- Consider posting a discussion thread in Q&A category before tagging,
  so consumers can flag concerns

### 1.0.0 (deferred — Phase 14 / ~Dec 2026)

Per D-08b, 1.0.0 requires ALL of:

- 2 consecutive 0.x.y minors without breaking-change pressure
- ≥3 external consumers (corporate / hobbyist / academic)
- learnforge-core test coverage ≥80%
- WASM target compiles on CI matrix (not just one-time)
- Phase 9 web platform consumption stress-tests pass

Don't ship 1.0.0 ahead of these gates. The big-launch coordination
(HN/Reddit/Lobsters per D-04b deferred) lines up with 1.0.0 timing.

---

## Troubleshooting reference

### `cargo publish` fails with "crate version already uploaded"

You are running the bootstrap publish a second time. crates.io rejects
duplicate version uploads. Either:

- Yank the existing version (`cargo yank learnforge-core@0.1.0`) and
  publish 0.1.1 instead, or
- Recognize that the bootstrap publish already succeeded and skip this
  step.

### `core-publish.yml` fails at OIDC token exchange

Trusted Publishing is not yet configured for the crate, OR the
repository / workflow / environment values do not match exactly.
Re-check the crates.io UI Trusted Publishers row against
`core-publish.yml`'s `name`, `on.push.tags`, and `environment` fields.

### macOS notarization fails with "invalid signing identity"

The `APPLE_SIGNING_IDENTITY` secret does not match the certificate's
common name. Run on your local machine:

```bash
security find-identity -v -p codesigning
# Look for the line ending in "Developer ID Application: ... (TEAMID)"
# Copy the full string between quotes — that is the secret value.
```

### release.yml succeeds but no GitHub Release appears

`tauri-action` creates the release as a draft. Visit
https://github.com/agentixgarage/learnforge/releases and look for a
draft entry. Click it to review and publish.

### Discussion template doesn't render

Slug mismatch — see Pitfall 4 in RESEARCH and Step 2 of the
Discussions section above. The filename
`.github/DISCUSSION_TEMPLATE/q-a.yml` must match the category's URL
slug exactly.

### Outreach message accidentally cc'd multiple recipients

This is a D-04c policy violation. Apologize to recipients; do not
re-send. Going forward, use one recipient per send.

---

## References

- [Phase 8 acceptance walkthrough](../.planning/phases/08-publishing/08-ACCEPTANCE.md)
- [Phase 8 Wave 7 plan](../.planning/phases/08-publishing/08-07-PLAN.md)
- [Phase 8 research (RESEARCH.md)](../.planning/phases/08-publishing/08-RESEARCH.md)
- [Wave 6 macOS signing setup](./MACOS-SIGNING-SETUP.md)
- [VERSIONING.md (D-03 policy)](./VERSIONING.md)
- [SECURITY.md](../SECURITY.md)
- [Top-level CHANGELOG.md](../CHANGELOG.md)
- [Per-crate CHANGELOG.md](../learnforge-core/CHANGELOG.md)
- [OUTREACH.md template](../.planning/phases/08-publishing/08-07-OUTREACH.md)
