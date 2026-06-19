# Licensing

This repository hosts two products under different licenses. Read
this file before assuming what you can do with the code.

## TL;DR

| What | License | Can you use it? |
|------|---------|-----------------|
| Everything OUTSIDE `pro/` | **MIT** (`LICENSE`) | Yes — commercially, freely, with attribution |
| Anything INSIDE `pro/` | **Proprietary** (`LICENSE-STUDIO`) | Read + audit only; commercial use requires a paid license key |
| `docs/studio-*` files | **Proprietary** (`LICENSE-STUDIO`) | Read + audit only |
| `build.config.ts` | **Proprietary** (`LICENSE-STUDIO`) | Read + audit only |
| Whitepapers in `docs/` and `learnforge-core/docs/` | **CC BY 4.0** | Yes — with attribution |

## Two products

This single repository builds two distinct products:

### LearnForge (open source)

- **License:** MIT (full text: `LICENSE`)
- **Audience:** Individual learners, hobbyists, students
- **Distribution:** GitHub Releases (desktop binaries) + crates.io
  (`learnforge-core` library)
- **What you can do:** Anything. Fork, modify, redistribute, sell,
  vendor, ship a competing product. MIT is maximally permissive.

### LearnForge Studio (commercial)

- **License:** Proprietary (full text: `LICENSE-STUDIO`)
- **Audience:** Engineering teams, L&D departments, enterprises
- **Distribution:** Direct sales (Phase 14+); license-key gated
- **What you can do:**
  - Read + audit the source freely (transparency for security review)
  - Use personally with a valid license key
  - Use commercially with a paid commercial license key
  - Fork + modify for non-commercial use (research, learning, audit)
  - Contribute back via the Studio Contributor Agreement (see
    `docs/CONTRIBUTING-STUDIO.md`)
- **What you CANNOT do:**
  - Use commercially without a license key
  - Fork + sell as a competing product
  - Tamper with the license-key validation logic
  - Remove or alter the copyright notices

If LICENSE-STUDIO and this file disagree, LICENSE-STUDIO wins.

## Why this dual license?

Open-core. Pattern matches GitLab CE/EE, PostHog, Cal.com, Supabase,
Mattermost — all viable single-repo open-source projects that also
sell a commercial tier.

The OSS side (`LearnForge`) is the full adaptive-learning desktop
app — usable end-to-end by an individual learner without ever paying.

The commercial side (`LearnForge Studio`) adds enterprise-tier
features: multi-tenancy, cohort management, managed AI billing,
org-branded certificates, SSO, audit logging, SOC 2 readiness, etc.
These features matter to L&D departments and enterprise procurement,
not to a single learner studying at home.

See `docs/OSS-VS-STUDIO.md` for the full feature placement matrix.

## How to tell which license governs a file

Two rules:

### Rule 1 — Path-based default

| Path | License |
|------|---------|
| `pro/**` | LICENSE-STUDIO |
| `docs/studio-*` | LICENSE-STUDIO |
| `build.config.ts` | LICENSE-STUDIO |
| `learnforge-core/docs/*.md` | CC BY 4.0 (whitepapers) |
| `docs/blog/*.md` | CC BY 4.0 (when so noted in the file footer) |
| Everything else | MIT |

### Rule 2 — File header authoritative override

If a source file declares a license in its header, that header wins
over the path-based default. Authors are encouraged to include explicit
headers — but absence of a header does NOT change governance.

Examples:

```rust
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 LearnForge contributors

// or

// LICENSE-STUDIO — Proprietary
// Copyright (c) 2026 LearnForge Studio
// All rights reserved. See LICENSE-STUDIO at repository root.
```

## License keys

The Studio binary requires a valid license key at runtime to enable
commercial-tier features (Phase 14 implements; current scaffold lets
it boot in dev mode for maintainers).

License keys are:
- Ed25519-signed by LearnForge Studio's private signing key
- JWT-encoded with expiration + seat count + tier metadata
- Validated at app startup; offline-grace period of 7 days
- Issued via direct sales (contact `hello@learnforge.dev`)

Tampering with the license-key validation logic is a violation of
LICENSE-STUDIO and may constitute computer-fraud under applicable
laws (United States CFAA, India IT Act 2000 §66, EU Directive
2013/40/EU).

## Contributing

- **OSS contributions** (anything outside `pro/`): see `CONTRIBUTING.md`.
  Standard CLA via `.github/workflows/cla.yml` (Phase 03.2 wired CLA
  Assistant Lite v2.6.1).

- **Studio contributions** (anything inside `pro/` or LICENSE-STUDIO
  files): see `docs/CONTRIBUTING-STUDIO.md`. Restricted to authorized
  contributors who have signed the Studio Contributor Agreement.

External pull requests touching `pro/` are auto-closed by maintainers.

## Related licenses by component

- **`learnforge-core` Rust crate (on crates.io):** MIT only. Always.
  Never proprietary. The crate is published independently and may be
  consumed by any project under any license.

- **Algorithm whitepapers** (e.g. `learnforge-core/docs/BKT.md`,
  `SM2.md`, `THRESHOLD.md`, `MICROLEARNING.md`, `SIGNING.md`):
  CC BY 4.0. Anyone can republish with attribution.

- **Topic packs** (`topic-packs/`): MIT for the format, schema, loader,
  and currently bundled packs. Future Studio-tier packs (Phase 11+)
  may carry LICENSE-STUDIO; the loader will gate them via a
  `requires_license: bool` field in pack.json.

- **Third-party dependencies:** see individual `Cargo.toml` /
  `package.json` files for transitive license info. Bundled
  third-party code retains its original license.

- **DeepTutor-derived code** (where present): Apache 2.0 with
  attribution per `THIRD_PARTY_NOTICES.md`.

## Reporting license violations

If you believe a fork, distribution, or commercial use violates
LICENSE-STUDIO, contact `hello@learnforge.dev` with:

- The violating product / repository / domain
- Evidence of LICENSE-STUDIO covered code being used commercially
  without a valid license key
- Date of discovery

Good-faith bug reports about license-key validation issues should go
to `security@learnforge.dev` per `SECURITY.md`.

## Questions

Email `hello@learnforge.dev` for licensing-related questions. We
respond within 5 business days.

---

*See also: `LICENSE` (MIT), `LICENSE-STUDIO` (proprietary),
`docs/OSS-VS-STUDIO.md` (feature placement),
`docs/CONTRIBUTING-STUDIO.md` (Studio contribution rules),
`CONTRIBUTING.md` (OSS contribution rules), `SECURITY.md`.*
