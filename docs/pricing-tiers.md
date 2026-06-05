<!--
Copyright (c) 2026 LearnForge Studio. All rights reserved.
This file is part of LearnForge Studio and is proprietary software.
Unauthorized copying, distribution, or modification is prohibited.
See LICENSE-STUDIO in the repository root for terms.
-->

# LearnForge Studio — Pricing Tiers

> **Source of truth.** Locked at Phase 03.2 strategy session
> 2026-06-03. Changes require a PRD update + repricing memo.

## Model

- Per-seat annual subscription
- Named seats (one person per seat per term)
- Volume-tiered: discount kicks in at 100 and 1,000 seats
- Annual billing only (no monthly — Phase 14 may revisit)

## Tiers (locked 2026-06-03)

| Tier        | Seats        | Price (per seat / year) | Annual contract (representative)        |
|-------------|--------------|-------------------------|-----------------------------------------|
| Team        | 10 – 99      | $79                     | 10 seats = $790; 50 seats = $3,950      |
| Business    | 100 – 999    | $59                     | 100 seats = $5,900; 500 seats = $29,500 |
| Enterprise  | 1,000+       | ~$39 (negotiated)       | Annual contract negotiated per deal     |

## Why these prices

- **Team ($79)**: roughly half the per-seat cost of Pluralsight's
  Standard tier ($179 USD/year listed), because LearnForge Studio
  replaces vendor-curated video with adaptive engineering practice
  — the value is hands-on lab time and measurable mastery, not
  content count. Below $79 we cannot fund AI cost + dev velocity at
  Team scale.
- **Business ($59)**: ~25% discount at 100+ seats. Customer becomes
  a reference + contributes feature requests + their org's data
  improves the BKT calibration (anonymized, opt-out).
- **Enterprise (~$39, negotiated)**: 50% discount at 1,000+ seats
  in exchange for a multi-year contract, security review, and a
  single billing channel. Final number depends on deployment
  footprint (managed AI vs BYOK, SLA tier, SSO integration).

## Add-ons (Phase 14+)

- **Managed AI**: $20/seat/year added to base tier — AI cost is
  absorbed by Studio rather than passed through to customer's
  Anthropic/OpenAI bill. Customer wants predictable billing.
- **SSO / SAML**: included Business+; Team can add for $1,000/year
  flat.
- **Custom topic packs**: $5,000 setup + included in subscription
  (Enterprise only).

## What's intentionally NOT priced

- **Cohorts**: deferred to v2.0 per CONTEXT.md. Lightweight
  "Groups" ship in Studio as an unbilled feature first to validate
  demand before any cohort SKU.
- **xAPI / SCORM / LTI export**: included in Business+ at no
  additional cost (Phase 12). Adoption helps Studio win deals
  where existing LMS investment is large.

## Floor and ceiling

- **Floor**: $39 / seat / year. Below this we cannot sustain
  development at Studio's level of engineering investment.
- **Ceiling**: $200 / seat / year (Team tier, individual contract).
  Above this we are competing on procurement complexity, not
  value — wrong market.

## Discount policy

- Annual prepay only (no monthly billing).
- 5% additional discount for multi-year prepay (Business+).
- Non-profits and accredited educational institutions: 50% off
  published Business pricing.
- Startup discount (Series A or earlier, <50 employees): one-year
  Team tier at $39/seat/year (matches Enterprise floor).
