# Security Policy

SkillCoco ships an open-source desktop application plus the
`learnforge-core` Rust crate. This policy describes how to report a
vulnerability in either component, what response you can expect, and
how we coordinate disclosure.

## Supported Versions

Pre-1.0 versions receive security patches only for the most recent
minor release. After 1.0.0 ships (target approximately December 2026
per Phase 8 D-08), we will maintain at least the two most recent minor
releases.

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | supported          |
| < 0.1   | not supported      |

The same support window applies to both `learnforge-core` (crates.io)
and the SkillCoco desktop application (GitHub Releases). Their
versions may diverge at the patch level per D-03c, but the supported
minor track is shared.

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub
issues, discussions, or pull requests.**

Use one of these private channels instead:

1. **GitHub Private Vulnerability Reporting (preferred):** open
   <https://github.com/skillcoco/skillcoco/security> and click
   "Report a vulnerability." This routes the report directly to the
   maintainers as a private security advisory draft and is the
   fastest path to a CVE if one is warranted.
2. **Email (fallback):** `hello@initcron.org`. Use this if you do
   not have a GitHub account or the Security tab is unavailable to
   you. Encrypt sensitive payloads at your discretion; we do not
   currently publish a GPG key — request one in your initial email if
   needed.

Please include in your report:

- A description of the issue and its security impact
- Steps to reproduce (ideally a minimal proof-of-concept)
- Affected versions (`learnforge-core` crate version, desktop app
  version, host OS where relevant)
- Any known mitigations or workarounds
- Whether you would like credit in the advisory (we default to
  opt-out unless you explicitly request acknowledgement)

## Disclosure Policy

We will **acknowledge your report within 3 working days**.

If the issue is confirmed as a vulnerability, we will:

1. Open a draft [GitHub Security Advisory](https://docs.github.com/en/code-security/security-advisories)
   in the `skillcoco/skillcoco` repository and invite you as a
   collaborator.
2. Develop a fix in a temporary private fork attached to that
   advisory.
3. Request a CVE identifier via GitHub's CNA (typical turnaround is
   approximately 72 hours).
4. Coordinate disclosure timing with you.

This project follows a **90-day coordinated disclosure timeline**
(D-07b). We aim to ship the fix and publish the advisory before that
window expires; if more time is needed for a complex remediation we
will request an extension with the reporter.

Pre-disclosure pre-publication coordination — embargoed details
shared with downstream integrators ahead of a public advisory — is
available on request through either channel above. Reach out before
posting the public advisory if you need this coordination.

## Acknowledgments

Reporters who request credit will be acknowledged in the published
GitHub Security Advisory and in the patch release's CHANGELOG entry.
Hall-of-fame style acknowledgement on a separate page may be added
in a future minor release.

## Secret Rotation

The release pipeline relies on the following GitHub repository
secrets, which are added in Phase 8 Wave 6 (macOS code-signing +
notarization). They are encrypted at rest by GitHub per D-02c and
rotated **annually** (or immediately on suspected compromise).

| Secret                       | Purpose                                                 |
| ---------------------------- | ------------------------------------------------------- |
| `APPLE_CERTIFICATE`          | Base64-encoded .p12 Developer ID certificate            |
| `APPLE_CERTIFICATE_PASSWORD` | Password protecting the .p12                            |
| `APPLE_SIGNING_IDENTITY`     | Codesign identity string ("Developer ID Application: ...") |
| `APPLE_ID`                   | Apple ID used for notarization submission               |
| `APPLE_PASSWORD`             | Apple ID app-specific password (NOT the account password) |
| `APPLE_TEAM_ID`              | Apple Developer Team ID                                 |

Rotation procedure (annual cadence, also triggered ad-hoc on
suspected compromise):

1. Generate a fresh Developer ID certificate in the Apple Developer
   portal; export as `.p12` with a strong password.
2. Generate a fresh Apple ID app-specific password at
   <https://appleid.apple.com>.
3. Update all six secrets in the GitHub repository settings.
4. Trigger a no-op release-staging workflow run to verify the new
   credentials before the next real release.
5. Revoke the previous certificate in the Apple Developer portal.

The crates.io publish workflow uses **Trusted Publishing** (OIDC,
per D-06c) — no static `CARGO_REGISTRY_TOKEN` secret is stored at
rest, eliminating that class of long-lived credential entirely.

## Scope

In scope for this policy:

- `learnforge-core` crate (Rust library published to crates.io)
- SkillCoco desktop application (Tauri 2, distributed via GitHub
  Releases on macOS, Linux, and Windows)
- The `skillcoco/skillcoco` repository itself, including CI
  workflows under `.github/workflows/`

Out of scope:

- The closed-source `SkillCoco Studio` (paid tier) — report Studio
  vulnerabilities via the same channels above; they are handled
  under a separate disclosure track.
- Third-party dependencies (please report upstream first; we will
  pick up advisories via Dependabot).
- Vulnerabilities requiring physical access to the local user's
  machine where a baseline OS-level threat model would already
  consider the device compromised.

---

*Last reviewed: 2026-07-12 (Phase 19 security audit — policy unchanged; see `.planning/SECURITY-OVERVIEW.md` for the per-phase threat register rollup).*
