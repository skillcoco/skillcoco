<h1 align="center">LearnForge</h1>

<p align="center">
  <strong>Mastery-driven learning for engineering teams. Open source. Built on real learning science.</strong>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="MIT" /></a>
  <a href="LICENSE-DOCS"><img src="https://img.shields.io/badge/Docs-CC_BY_4.0-orange.svg" alt="CC BY 4.0" /></a>
  <img src="https://img.shields.io/badge/Tauri-2.0-FFC131?logo=tauri&logoColor=white" alt="Tauri 2" />
  <img src="https://img.shields.io/badge/Rust-stable-DEA584?logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/React-18-61DAFB?logo=react&logoColor=white" alt="React 18" />
  <img src="https://img.shields.io/badge/Status-Active_Development-success" alt="Active development" />
</p>

<p align="center">
  <em>Replaces "did they watch the video?" with "can they actually do this?" — and adapts the entire course around the answer.</em>
</p>

```bash
git clone https://github.com/agentixgarage/learnforge && cd learnforge && pnpm install && pnpm tauri dev
```

---

LearnForge is an **open-source adaptive learning platform** for engineering teams. It combines **Bayesian Knowledge Tracing**, **SM-2 spaced repetition**, **AI-generated content shaped by learner state**, and **hands-on terminal labs** in a single mastery loop. Modules don't unlock by time-in-seat — they unlock when the learner *demonstrates* understanding. Free desktop app today (MIT). Cohort-grade web platform for L&D in a few months.

> If your organization spends six figures a year on L&D and cannot answer *"how many of our engineers can actually do X today?"* — we built this for you.

---

## The adaptive mastery loop

```
Assess  →  Generate path  →  Module content  →  Practice + lab
   ↑              ↓                  ↓                  ↓
   └──── Path adapts ──── BKT mastery ──── SM-2 review queue
```

**One loop, two algorithms, two mastery dimensions:**
- **Conceptual mastery** (BKT, fed by quiz + flashcards) — *I know it.*
- **Practical mastery** (linear, fed by lab steps completed) — *I can do it.*

Modules unlock when both mastery dimensions cross threshold (or just one — it's per-module configurable). SR cards auto-generate when a module is mastered. Reviews land at scientifically-optimal intervals. The whole course reshapes around each learner.

---

## What's inside

### Adaptive learning model
- **BKT × SM-2 dual loop** — Carnegie-Mellon's mastery algorithm meets Anki's retention algorithm in one closed-loop system
- **Two mastery dimensions** — conceptual + practical, separately measured, configurably gating
- **DAG-based skill trees** — multiple valid routes, prerequisite-gated unlocks, diamond dependencies first-class
- **Block taxonomy** — `section` / `text` / `callout` / `quiz` / `flash_cards` / `lab` — every block produces a separate mastery signal
- **Skeleton-then-fill** — block rows insert as `pending`, transition through `generating` to `ready`; one failed block doesn't break a module

### AI / LLM
- **PagePlanner architecture** — one LLM call decomposes a module into 8-10 lessons + 5-10 quiz Qs + flashcards + lab specs; per-block parallel generation via `Semaphore(3)`
- **Content shaped by real learner state** — module 5's bytes literally differ between learners based on BKT scores from modules 1-4
- **AI-judge step evaluator** — last-resort LLM grading for open-ended steps with per-session budget (5 calls + 2s cooldown) and graceful no-auth fallback
- **OAuth-first AI integration** — sign in with Claude Pro / ChatGPT Plus / Gemini Advanced; BYOK or local Ollama always available
- **Topic-pack override system** — curated `manifest.yaml` lab specs replace AI-generated default per-module
- **AI-generated Dockerfile escape hatch** — for novel topics, PagePlanner emits container spec inline with the lab
- **Two-file persistent learner memory** *(Phase 2)* — `PROFILE.md` + `SUMMARY.md` rewritten by LLM with `NO_CHANGE` sentinel, injected into every prompt
- **Draft → Critique → Revise path generation** *(Phase 2)* — three-call LLM critique loop catches missing prerequisites, cycles, redundancy
- **Externalized YAML prompts** *(Phase 2)* — versioned, language-fallback, swappable per-domain
- **Concept graph with rationale** *(Phase 2)* — typed nodes/edges with `depends_on | extends | related` plus explanation strings, rendered as Mermaid

### Hands-on practice
- **Embedded PTY terminal** — `portable-pty` + xterm.js v5 wired through Tauri events; cross-platform (macOS/Linux/Windows ConPTY)
- **Hybrid sandbox** — Docker container per lab via `bollard` when available; host-shell fallback when not; per-lab `requires_docker` override
- **Inline step evaluation** — OSC 133 prompt-boundary detection + heuristic regex fallback + manual recheck floor; three-layer resilience
- **Four step-check kinds** — `command_regex`, `exit_code`, `file_state`, `ai_judge` (last resort, budget-guarded)
- **Cumulative-within-module workspace** — `~/.learnforge/labs/<track>/<module>/` bind-mounted into the container; lab 2 sees lab 1's files
- **Surgical reset** — only files declared in `creates: []` are wiped; sibling work preserved; path-traversal guarded
- **Cross-restart resume** — current step + completed step IDs + AI-judge verdicts persist in `lab_progress`; close the laptop, pick up tomorrow
- **3-tier progressive hints** — gentle nudge → partial answer → full solution, manually revealed
- **`LAB.md` authoring** — Markdown body + YAML frontmatter; AI-generated by default, human-curated when it matters

### Engineering & methodology
- **Embedded intelligence, no cloud** — RuVector (vector + graph DB, in-process) + SQLite WAL + in-process BKT/SM-2 + ~10 MB Tauri binary
- **`learnforge-core` extractable Rust crate** *(Phase 7)* — same algorithms, same crate, powers desktop + web + any third-party embed
- **Determinism-first testing** — trait-based mocks (`LabRuntime`, `DockerProbe`, `AIClientTrait`) — 100% of unit/integration tests run with **zero real Docker, zero real PTY, zero real LLM**
- **TDD London discipline** — Wave-0 RED tests precede every implementation wave; surfaced and closed 5 cross-layer integration gaps in Phase 03.1
- **camelCase IPC contract, type-checked** — `#[serde(rename_all = "camelCase")]` everywhere; Rust↔TS schema mismatches fail at compile
- **Auditable migration framework** — versioned, idempotent, atomic-tx-wrapped (`v001` → `v006`); each migration ships with a self-test
- **Ruflo-orchestrated execution** — every phase ships with CONTEXT → RESEARCH → PLAN → VERIFY → EXECUTE → ACCEPT trail. Open methodology, not just open code.

---

## The science (auditable)

| Algorithm | Source | Code |
|-----------|--------|------|
| Bayesian Knowledge Tracing | Corbett & Anderson, 1994 | [`learning/adaptive.rs`](src-tauri/src/learning/adaptive.rs) |
| SM-2 spaced repetition | Wozniak, 1990 | [`learning/spaced_repetition.rs`](src-tauri/src/learning/spaced_repetition.rs) |
| Forgetting curve | Ebbinghaus, 1885 | (drives SR scheduling) |
| Block taxonomy + concept graph | DeepTutor (HKUDS, Apache 2.0) | [`db/blocks.rs`](src-tauri/src/db/blocks.rs) |

Read it. Audit the math. Calibrate it. The opposite of opaque AI scoring you cannot defend.

---

## Topic packs

| Pack | Status |
|------|--------|
| Kubernetes (CKA-aligned), Terraform, Rust, Go, Python | Shipping |
| Agentic DevOps, AI Engineering | Q3 2026 |

Topic packs are **structure**, not video. AI generates content per learner against the pack's skill tree. Curated labs override per module.

---

## Open core

**Free desktop (MIT — this repo):** full adaptive engine, hands-on labs, BYOK or Ollama, all algorithms auditable, data stays local.

**LearnForge Cloud (commercial — late 2026):** multi-tenant web · cohorts · manager heatmaps · multi-modal video + labs · SSO/SAML · SCORM/xAPI · Credly badges · ROI analytics · white-label. Same `learnforge-core` crate. Algorithms remain open; the moat is enterprise surface.

→ **L&D leaders at GCCs and software orgs:** [open an issue tagged `pilot-interest`](https://github.com/agentixgarage/learnforge/issues/new) to be a design partner.

---

## Use cases

- **Cloud / Kubernetes / Terraform upskilling** at scale, with measurable mastery per learner
- **AI engineering bootcamps** with hands-on labs (real LLM APIs, agents, evaluation harnesses)
- **DevSecOps / Platform Engineering ramp-up** with DAG-captured prerequisites
- **GCC new-hire onboarding** — adaptive 2-week paths replacing 4-week classroom training
- **Compliance training that retains** — SR auto-generation makes 90-day stickiness real

---

## Quick start

```bash
# Prerequisites: Node 18+, pnpm 8+, Rust stable, Tauri 2 prerequisites
# Optional: Docker (labs work in host-shell mode without it)

git clone https://github.com/agentixgarage/learnforge.git
cd learnforge
pnpm install
pnpm tauri dev          # development
pnpm tauri build        # production binary
pnpm test               # frontend tests
(cd src-tauri && cargo test)   # backend tests
```

---

## Roadmap

- **v1.0** *(Dec 2026)* — Open-source desktop. Adaptive loop closed. Hands-on labs shipping. Topic packs · microlearning · certification · web companion preview.
- **v1.1** *(Q1 2027)* — Corporate foundation. Multi-tenant cloud · cohorts · manager dashboards · multi-modal video + labs · gamification · Credly.
- **v2.0** *(mid-2027)* — Enterprise. SSO/SAML · audit logging · SOC 2 · ROI analytics · SCORM/xAPI · custom topic packs · white-label.

Phase-level detail: [`.planning/ROADMAP.md`](./.planning/ROADMAP.md). Per-requirement traceability: [`.planning/REQUIREMENTS.md`](./.planning/REQUIREMENTS.md).

---

## Built by

**[Gourav Shah](https://www.linkedin.com/in/gouravshah/)** — Founder, [School of DevOps](https://schoolofdevops.com) and [Agentix Garage](https://agentixgarage.com). 17+ years in DevOps · 270K+ Udemy students · trained engineers at Adobe, Cisco, Visa, Walmart Labs, IBM, Expedia, DreamWorks, EMC², RBS, Accenture, Nasdaq, Volkswagen, NetApp.

**Vivian Aranha** — Data & AI Specialist at IBM, CEO of School of AI · MIT Sloan · GWU · 1M+ Udemy enrollments · 8+ years in AI/ML.

1.27M combined enrollments. We're building the platform we wish existed when we were teaching.

---

## License

**Code** MIT · **Docs & algorithms** CC BY 4.0 · **Third-party attributions** [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)

Adaptive learning algorithms should be open, auditable, and patent-free. That is the design.

---

<p align="center"><em>An Agentix Garage and School of DevOps / School of AI project. Built in the open since March 2026.</em></p>
