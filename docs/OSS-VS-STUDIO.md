# OSS vs Studio — Feature Placement Matrix

**Status:** Source of truth for "which features ship to which product"
**Last updated:** 2026-06-19
**Authority:** Locked decisions from 2026-06-19 architecture review

This document is the authoritative reference for whether a feature lives
in the OSS desktop app (`LearnForge`, MIT) or the commercial overlay
(`LearnForge Studio`, proprietary). When in doubt, this file wins.
Update via PR with maintainer approval before changing placement.

---

## Decision #0 — Strategic stance

**OSS adoption first; Pro revenue follows scale.**

Pattern: PostHog, Cal.com, Supabase, Mattermost. Win community over
6-12 months; Pro revenue scales from corporate users WITHIN the
community.

Implications:
- **Default to OSS** when in doubt. A feature moves to Pro only with
  clear corporate-value justification.
- **Don't gate viral surfaces.** Certs (unsigned), shareable artifacts,
  terminal labs, topic packs all stay OSS.
- **Pro tier = enterprise infrastructure**, not algorithmic
  improvements. Multi-tenant, audit logs, SSO, org branding, managed
  services.
- **Generous OSS scope.** Free OSS = full single-user experience.
- **Anti-pattern:** never switch OSS to restrictive license later.
  Starting open-core ≠ switching open-core. We start cleanly.

Anti-pattern reference (do NOT repeat):
- Terraform → BSL → OpenTofu fork
- MongoDB → SSPL → DocumentDB fork
- Elastic → ELv2 → OpenSearch fork
- Redis → SSPL → Valkey fork

Each lost 30-50% of community within 6 months. License-switch is the
single largest reason open-source projects fork.

---

## Two products, one codebase

| | LearnForge | LearnForge Studio |
|--|------------|-------------------|
| **Product name** | LearnForge | LearnForge Studio |
| **License** | MIT | Proprietary (LICENSE-STUDIO) |
| **Audience** | Individual learners, hobbyists, students | Engineering teams, L&D departments |
| **Binary** | `learnforge` (Tauri 2 desktop) | `learnforge-studio` (Tauri 2 desktop) |
| **Build flag** | default | `LEARNFORGE_PRO=1` |
| **Distribution** | GitHub Releases + crates.io | Direct sales (Phase 14+) |
| **Pricing** | Free | Per-seat: Team $79 / Business $59 / Enterprise ~$39 |
| **Auth** | None / BYO API key | License key (Ed25519 + JWT) |
| **Multi-user** | Single learner | Multi-tenant (Phase 10+) |

Both binaries built from the same git repository.
Both consume the same `learnforge-core` crate from crates.io.
The Studio binary adds a `pro/` overlay that registers additional
Tauri commands + React components via the `LearnForgePlugin` trait +
`@pro` Vite alias.

---

## Feature placement matrix

Columns:
- **OSS** = ships in the free `LearnForge` desktop app (MIT)
- **Studio** = ships in `LearnForge Studio` only (proprietary)
- **Phase** = where the feature lands in the roadmap

### Adaptive engine (the core learning loop)

| Feature | OSS | Studio | Phase |
|---------|-----|--------|-------|
| Bayesian Knowledge Tracing (BKT) mastery | ✓ | inherits | Phase 1 |
| SuperMemo-2 spaced repetition | ✓ | inherits | Phase 1 |
| Module unlock DAG | ✓ | inherits | Phase 1 |
| AI-generated learning paths | ✓ | inherits | Phase 1 |
| `learnforge-core` Rust crate | ✓ (MIT, crates.io) | inherits | Phase 7 |
| BYO API key (Anthropic / OpenAI / Gemini) | ✓ | ✓ | Phase 1 |
| **Managed AI** (cost absorbed) | — | ✓ | Phase 14 |

### Content + structure

| Feature | OSS | Studio | Phase |
|---------|-----|--------|-------|
| Block taxonomy (section / quiz / flash_cards / callout) | ✓ | inherits | Phase 3 |
| Per-block mastery signals | ✓ | inherits | Phase 3 |
| Page Planner (AI lesson decomposition) | ✓ | inherits | Phase 3 |
| Module persistence (SQLite) | ✓ | inherits | Phase 3 |
| Tutor sidebar (grounded in current lesson) | ✓ | inherits | Phase 3 |

### Hands-on labs (Phase 03.1)

| Feature | OSS | Studio | Phase |
|---------|-----|--------|-------|
| `lab` block type | ✓ | inherits | Phase 03.1 |
| Embedded PTY-backed terminal | ✓ | inherits | Phase 03.1 |
| Docker / host shell runtime selector | ✓ | inherits | Phase 03.1 |
| LAB.md spec format + 4-kind step evaluator | ✓ | inherits | Phase 03.1 |
| 3-tier progressive hints | ✓ | inherits | Phase 03.1 |
| Practical-required gating | ✓ | inherits | Phase 03.1 |
| **K8s / cloud sandbox runtime** | — | ✓ (future) | Phase 14+ |
| **Lab audit log (org-wide)** | — | ✓ (future) | Phase 11+ |

**Rationale:** Hands-on terminal is a viral differentiator vs other OSS
adaptive learning. Codecademy gives free terminals; we should too.
Lab CONTENT can be Pro-only (corporate-vetted compliance labs), but
the lab RUNTIME is OSS.

### Microlearning (Phase 4)

| Feature | OSS | Studio | Phase |
|---------|-----|--------|-------|
| Daily challenge surface | ✓ | inherits | Phase 4 |
| BKT-decay + SR-due selection algorithm | ✓ | inherits | Phase 4 |
| Global daily streak | ✓ | inherits | Phase 4 |
| Dashboard "Today's challenge" card | ✓ | inherits | Phase 4 |
| Auto-enable on first mastered module | ✓ | inherits | Phase 4 |
| **Push notifications / OS reminders** | — | ✓ (future) | Phase 14+ |
| **Cohort-shared streaks** | — | ✓ (future) | Phase 11+ |

### Topic packs (Phase 5)

| Feature | OSS | Studio | Phase |
|---------|-----|--------|-------|
| Pack JSON schema (Draft 2020-12) | ✓ | inherits | Phase 5 |
| Pack loader (bundled + ~/.learnforge/skills/) | ✓ | inherits | Phase 5 |
| Schema validator | ✓ | inherits | Phase 5 |
| Skills system (user-authored packs) | ✓ | inherits | Phase 5 |
| Settings → Topic Packs UI | ✓ | inherits | Phase 5 |
| Onboarding pack picker | ✓ | inherits | Phase 5 |
| Six bundled packs (K8s/Rust/Go/Python/Agentic DevOps/AI Engineering) | ✓ | inherits | Phase 5 |
| **Studio packs (license-key-gated)** | — | ✓ (future) | Phase 11+ |
| **Corporate compliance packs (SOC2, Security+, CKA prep, etc.)** | — | ✓ (future) | Phase 11+ |
| **Pack marketplace** | — | ✓ (future) | Phase 14+ |
| **Org-private pack distribution** | — | ✓ (future) | Phase 11+ |

**Rationale:** Pack format + loader are MIT primitives. Anyone can
write a pack. Future commercial packs gate via a `requires_license:
bool` field in pack.json; OSS loader rejects gated packs without a
Studio license key.

### Certification (Phase 6 — split)

This is where OSS-adoption-first matters most. Cert is the primary
**viral surface** (LinkedIn-shared certs = organic billboards).

| Feature | OSS | Studio | Phase |
|---------|-----|--------|-------|
| Mastery tracking (Associate/Practitioner/Professional levels) | ✓ | inherits | Phase 6 |
| In-app achievement badges | ✓ | inherits | Phase 6 |
| Dashboard "Achievements" section + `/achievements` route | ✓ | inherits | Phase 6 |
| TrackView certification progress indicator | ✓ | inherits | Phase 6 |
| PackPicker certifications preview | ✓ | inherits | Phase 6 |
| **Unsigned PDF completion certificate** | ✓ | inherits | Phase 6 |
| **Unsigned PNG badge** | ✓ | inherits | Phase 6 |
| Copyable share text ("I just earned X on LearnForge") | ✓ | inherits | Phase 6 |
| **Ed25519 cryptographic signing** | — | ✓ | Phase 6 (split) |
| **QR code on certs** | — | ✓ | Phase 6 (split) |
| **Settings "Verify Certificate" panel** | — | ✓ | Phase 6 (split) |
| **Public verification URL (hosted)** | — | ✓ | Phase 14 |
| **Credly / Open Badges export** | — | ✓ | Phase 14 |
| **Org-branded certificate templates** | — | ✓ | Phase 14 |
| **Bulk issuance + audit log** | — | ✓ | Phase 11+ |
| **W3C Verifiable Credentials** | — | ✓ | Phase 14 |

**Rationale:** Free users get a shareable artifact that drives
virality (LinkedIn / Twitter / portfolio). Pro users get verifiable +
interoperable + org-branded credentials that hiring managers can
trust. Pattern matches Duolingo (free completion cert + paid verified
cert).

`learnforge-core::signing` + `learnforge-core::achievements` modules
remain published as **library primitives** (anyone can use). OSS
desktop binary simply does NOT import them in the cert generation
path; Studio binary does.

### Publishing + open-source launch (Phase 8)

| Feature | OSS | Studio | Phase |
|---------|-----|--------|-------|
| crates.io publication of `learnforge-core` | ✓ | inherits | Phase 8 |
| GitHub Releases automation | ✓ | inherits | Phase 8 |
| Algorithm whitepapers (BKT, SM2, threshold, microlearning, signing) | ✓ (CC BY 4.0) | inherits | Phase 7 + 8 |
| Launch blog articles | ✓ | inherits | Phase 8 |
| macOS code signing + notarization | ✓ | inherits | Phase 8 |
| SECURITY.md + GitHub Discussions + Issue templates | ✓ | inherits | Phase 8 |
| Versioning policy | ✓ | inherits | Phase 8 |

### Corporate foundation (Phase 10-14)

Everything below is Studio-only.

| Feature | OSS | Studio | Phase |
|---------|-----|--------|-------|
| Web app (hosted Studio) | — | ✓ | Phase 10 |
| Multi-tenant infrastructure | — | ✓ | Phase 10 |
| Managed AI billing | — | ✓ | Phase 14 |
| Cohort management (teams, assignments) | — | ✓ | Phase 11 |
| Manager dashboards | — | ✓ | Phase 11 |
| Multi-modal video content | — | ✓ | Phase 12 |
| Gamification (XP, leaderboards) | — | ✓ | Phase 13 |
| Credly / Open Badges export | — | ✓ | Phase 14 |
| Hosted certificate verification URL | — | ✓ | Phase 14 |
| Org-branded certificates | — | ✓ | Phase 14 |
| SSO / SAML / SCIM | — | ✓ | Phase 14 |
| Audit logging | — | ✓ | Phase 15 |
| SOC 2 Type II readiness | — | ✓ | Phase 14 |
| Advanced analytics (ROI, skill-gap) | — | ✓ | Phase 16+ |
| LMS integrations (SCORM / xAPI / LTI 1.3) | — | ✓ | Phase 17 |
| White-label + custom packs | — | ✓ | Phase 19 |

---

## Placement decision flowchart

When considering a new feature, walk this tree:

```
Is it a single-user / individual-learner feature?
├── YES → OSS by default
│   └── EXCEPT: corporate-vetted content (e.g. SOC2 training pack) → Studio
└── NO (involves multi-user, org-wide, hosted, audited, branded, or compliance)
    └── Studio
```

Specific tests to apply:
- Does a single hobbyist learner benefit standalone? → OSS
- Does it require server-side coordination across multiple users? → Studio
- Is it about org-wide reporting / audit / compliance? → Studio
- Is it a corporate purchase decision (procurement, security review)? → Studio
- Does it require a hosting service (URLs, dashboards, email) → Studio
- Could a competitor monetize this as a service? → Studio (license-gate it)

---

## Update protocol

Changes to this matrix require:
1. PR with maintainer review (both founders sign off)
2. Justification documented in the PR description
3. If moving Pro → OSS: announce in next release notes
4. If moving OSS → Pro: avoid retroactive removal; deprecate in OSS
   with at least one minor-version notice, then move to Pro in next
   major

**Never repeat the Phase 6 mistake of building features OSS-first
without explicit placement consideration.** Every new phase MUST
include placement debate before plan-phase begins.

---

*Authority: this document overrides any conflicting placement
references in PROJECT.md, ROADMAP.md, or phase CONTEXT.md files.*
