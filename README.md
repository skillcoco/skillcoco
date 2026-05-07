# LearnForge

**Mastery-driven learning for engineering teams.**
**Open source. Built on real learning science. Designed for enterprises that need their people to actually ship the new system — not just complete the course.**

---

## The problem L&D actually has

Your engineers complete training. Your LMS reports 95% completion rates. Your migration project still slips.

This is not a discipline problem. It's a measurement problem. Every major learning platform — Udemy Business, Coursera for Business, LinkedIn Learning, Pluralsight — measures progress by **video consumption**, not actual understanding. A learner who watched 100% of a Kubernetes course and a learner who can debug a production CrashLoopBackOff look identical in every dashboard.

Two failure modes follow:

1. **Completion is not competence.** Engineers finish courses they cannot apply at work. Your CTO can't tell who's actually ready to lead the cloud migration.
2. **Knowledge decays without intervention.** Without scientifically-timed review, **80% of what's learned is forgotten within 30 days** (Ebbinghaus, 1885). Every dollar spent on training that doesn't enforce spaced review is partially wasted on day 31.

For Global Capability Centers and software organizations onboarding hundreds of engineers onto Kubernetes, Terraform, or AI/ML in compressed timelines, this gap is no longer acceptable.

---

## What LearnForge does differently

LearnForge replaces *"did they watch the video?"* with *"can they actually do this?"* — and adapts the entire course around the answer.

### 1. Mastery-based progression, not time-in-seat

Every module ends gated by a **probabilistic mastery measurement** — Bayesian Knowledge Tracing (BKT), the same algorithm used in Carnegie Mellon's intelligent tutoring systems. Modules unlock only when the learner demonstrates real understanding, not when a timer expires.

A learner who already knows half the material moves through it in days. A learner who needs more practice gets it. Both reach the same competence bar.

### 2. Hands-on labs in real terminals

LearnForge ships an embedded PTY-backed terminal with sandboxed Docker labs. The learner reads the concept, opens the lab, types real commands (`kubectl apply -f pod.yaml`), and the system **automatically evaluates each step** using one of four check kinds — output regex, exit code, file state, or AI-judged for open-ended steps.

No multiple choice substituting for skill. No screenshots of someone else's terminal. Real shell, real commands, real evaluation.

When Docker isn't installed, the lab gracefully falls back to the host shell. Per-learner workspace persists across sessions; cross-restart resume is built in.

### 3. Two mastery dimensions: "I know it" and "I can do it"

Every module tracks **conceptual mastery** (quiz, flashcards, applied through BKT) and **practical mastery** (lab steps completed) as separate dimensions. Modules can require either, both, or just signal-track for analytics. A future certification phase will convert these dimensions into verifiable skill levels.

This is fundamentally different from "completed 47 of 50 videos." It's *"the engineer demonstrated they can deploy a multi-container Pod and recover from a failed rollout."*

### 4. AI-generated content shaped by learner state

Most "adaptive" platforms reorder pre-recorded content. LearnForge **generates each module's lessons at the moment the learner opens it**, using their actual BKT mastery from previous modules. If the learner struggled with networking in module 3, module 5's examples adjust accordingly.

Powered by direct API access to Anthropic Claude, OpenAI, Google Gemini, or local Ollama (offline / privacy-first). OAuth or BYOK — no API keys to manage if the learner already has a subscription.

### 5. Spaced repetition built in (not bolted on)

When a module is mastered, LearnForge auto-generates flashcards from key concepts and schedules them via **SM-2** — the same algorithm Anki uses. Reviews land at scientifically-optimal intervals, converting short-term understanding into lasting expertise. Every L&D leader who's watched skills evaporate three months after training will recognize why this matters.

---

## A learner's first ten minutes

```
1. Install LearnForge desktop app (single binary, macOS / Windows / Linux)
2. Pick a topic (Kubernetes, Terraform, Rust, Python, AI Engineering, ...)
3. State your goals + level — no Socratic interrogation, just a fast self-rating
4. Path generated — a DAG of modules with prerequisite edges visible
5. Open module 1 — content generates in ~30 seconds, personalized to your level
6. Read the lesson, take the quiz, run the lab
7. Watch BKT mastery move on the dashboard — actual percentage, not progress bar
8. Module 2 unlocks. SR cards appear in the review queue. Streak begins.
```

This is the **Definition of Usable** the project gates every phase against: install → topic → real learning → mastery moves, every time, no bugs.

---

## The science, made auditable

The algorithms in LearnForge are based on peer-reviewed research and shipped as auditable open-source code:

| Algorithm | Source | Where in code |
|-----------|--------|---------------|
| Bayesian Knowledge Tracing | Corbett & Anderson, 1994 | [`src-tauri/src/learning/adaptive.rs`](src-tauri/src/learning/adaptive.rs) |
| SM-2 Spaced Repetition | Wozniak, 1990 | [`src-tauri/src/learning/spaced_repetition.rs`](src-tauri/src/learning/spaced_repetition.rs) |
| Forgetting curve | Ebbinghaus, 1885 | (Drives SR scheduling) |
| Intelligent tutoring | VanLehn, 2011 | (Architectural reference) |
| Block taxonomy + concept graph | DeepTutor (HKUDS, Apache 2.0) | [`src-tauri/src/db/blocks.rs`](src-tauri/src/db/blocks.rs) |

Every line of the BKT update, SM-2 scheduler, and DAG path engine is in this repo, MIT-licensed, with unit tests. **Read it. Audit the math. Calibrate it for your domain.** This is the opposite of opaque AI scoring you cannot defend in an audit.

---

## Why open source matters here

### For technical buyers

- **No vendor lock-in.** The Rust core (`learnforge-core`) is being extracted into a publishable crate so any team can embed adaptive learning into their own platform.
- **Data sovereignty.** The desktop app stores everything locally — SQLite + RuVector embedded vector store. Nothing leaves the laptop unless you choose a cloud AI provider, and even then you can run Ollama locally for full offline operation.
- **Auditability.** Mastery decisions are explainable. Show your CISO the BKT formula, not a black-box score.

### For L&D buyers

- **Try before you buy at zero risk.** Free desktop app gives full access to the core adaptive engine. Pilot with one cohort before any procurement conversation.
- **Bottom-up adoption.** Engineers will install LearnForge themselves to learn Kubernetes; once it's in the org, the cohort web app becomes a natural upgrade for L&D-managed programs.
- **Algorithms you can defend.** When your VP asks why this person was certified L2 and that one wasn't, you have a peer-reviewed answer.

---

## Use cases

LearnForge is built for the topics where competence-vs-completion gaps are most expensive:

- **Cloud migration upskilling** — Hundreds of engineers learning Kubernetes, Terraform, AWS/GCP simultaneously, with measurable mastery per learner.
- **AI engineering bootcamps** — Hands-on labs with real LLM APIs, prompt engineering, agent frameworks. Practical mastery dimension is the differentiator.
- **DevSecOps / Platform Engineering ramp-up** — DAG paths capture prerequisite skills (Linux → Docker → K8s → mesh → GitOps) and progression is gated on real terminal exercises.
- **New hire onboarding at GCCs** — Replace 4-week classroom training with adaptive 2-week paths personalized to incoming skill level. Manager dashboards (v1.1) show cohort heatmaps.
- **Compliance training that retains** — SR card generation means audit-required training actually sticks 90 days later.

---

## Topic packs (shipping or imminent)

| Pack | Status | What you get |
|------|--------|--------------|
| Kubernetes (CKA-aligned) | Shipping | Pods → Deployments → RBAC → Networking → Storage → Scheduling — DAG with hands-on Kind labs |
| Terraform | Shipping | HCL → Backends → Modules → State manipulation → CI/CD patterns |
| Rust | Shipping | Ownership → Async → Traits → Error handling — code-eval exercises |
| Go | Shipping | Standard library → Concurrency → Testing — code-eval exercises |
| Python | Shipping | Idioms → Async → Testing — code-eval exercises |
| Agentic DevOps | Q3 2026 | LLM-powered ops, agent frameworks, observability for AI systems |
| AI Engineering | Q3 2026 | RAG, tool-use, agent design patterns, evaluation harnesses |

Topic packs are **structure**, not pre-recorded video — the AI generates content per-learner against the pack's skill tree.

---

## What's open and what's commercial

LearnForge follows an **open-core** model.

### Free (MIT-licensed) — this repo

- Tauri 2 desktop app (macOS, Windows, Linux)
- Full adaptive engine (BKT, SM-2, DAG paths, block taxonomy)
- AI integration with BYOK or Ollama
- Hands-on labs (PTY terminal + Docker / host-shell)
- Local SQLite storage; nothing leaves your machine
- All learning algorithms, auditable and patent-free

### Coming 2027 — LearnForge Cloud (commercial)

The same core, packaged for enterprise L&D:

- Multi-tenant web app with org accounts, SSO, RBAC
- **Cohort management** — assign paths, deadlines, manager visibility
- **Manager dashboards** — mastery heatmaps, completion-vs-mastery rates, at-risk learners
- **Multi-modal content** — video + text + code + labs in unified mastery model
- **Verifiable certification** — Credly / Open Badges integration, org-branded certificates
- **LMS integration** — SCORM / xAPI for compatibility with existing corporate LMS
- **Managed AI** — costs included in subscription, no learner BYOK required
- **Advanced analytics** — ROI reporting, skill-gap analysis, time-to-competence per cohort
- **API + webhooks** — programmatic access, custom topic packs, event-driven integrations

The same `learnforge-core` Rust crate powers both products. Algorithms remain open; the moat is enterprise feature surface, not lock-in.

If you're an L&D leader at a GCC, software org, or platform team and want to be a design partner for LearnForge Cloud, get in touch (contact links below). Early partners get extensive feature input and pilot pricing.

---

## Try it now

### Prerequisites

- Node.js 18+, pnpm 8+, Rust (stable), [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/)
- Optional for hands-on labs: Docker Desktop / colima / WSL2 (host-shell fallback works without)
- Optional for AI: a Claude / OpenAI / Gemini account, *or* Ollama for fully-local operation

### Install and run

```bash
git clone https://github.com/agentixgarage/learnforge.git
cd learnforge
pnpm install
pnpm tauri dev
```

### Build

```bash
pnpm tauri build      # Native installer for current platform
```

### Tests

```bash
pnpm test                                    # Frontend (Vitest)
(cd src-tauri && cargo test)                 # Backend (Rust)
```

---

## Architecture at a glance

```
┌─────────────────────────────────────────────────────────────────┐
│                       LearnForge Desktop                         │
├──────────────────────────┬──────────────────────────────────────┤
│      React 18 / TS       │         Tauri Backend (Rust)         │
├──────────────────────────┼──────────────────────────────────────┤
│                          │                                      │
│  Dashboard               │  Adaptive Engine                     │
│  Onboarding              │    BKT mastery tracking              │
│  ModuleView              │    SM-2 spaced repetition            │
│  ReviewSession           │    DAG path adaptation               │
│  TrackView (DAG vis)     │    Block taxonomy + per-block        │
│  Lab block (xterm.js)    │      mastery signals                 │
│  Settings                │                                      │
│                          │  AI Layer                            │
│  Zustand stores          │    Anthropic / OpenAI / Gemini       │
│   useLearningStore       │    Ollama (local, offline)           │
│   useLabStore            │    OAuth + BYOK                      │
│   useAIStore             │                                      │
│                          │  Hands-on Labs (Phase 03.1)          │
│  ↕ Tauri IPC             │    portable-pty terminal             │
│  (camelCase, type-safe)  │    bollard Docker integration        │
│                          │    Step evaluator (4 check kinds)    │
│                          │    OSC 133 prompt detection          │
│                          │                                      │
│                          │  Data Layer                          │
│                          │    SQLite (WAL mode, rusqlite)       │
│                          │    RuVector (vectors + graph DB)     │
└──────────────────────────┴──────────────────────────────────────┘
```

| Layer | Choice | Why |
|-------|--------|-----|
| Desktop | **Tauri 2** | Native binaries, ~10MB, system webview — not Electron |
| Backend | **Rust** | Embedded intelligence in-process, no GC pauses, sharable as crate |
| Frontend | **React 18 + TS** | Strict types across the IPC boundary |
| Storage | **SQLite (WAL)** | Concurrent reads, ACID, zero ops |
| Vector / Graph | **RuVector** | HNSW search + DAG storage, embedded |
| Terminal | **portable-pty + xterm.js** | Real PTY, cross-platform (macOS/Linux/Windows ConPTY) |
| Sandbox | **bollard (Docker) / chroot fallback** | Isolated lab containers when available, host-shell when not |
| Lab spec | **Markdown + YAML frontmatter** | Inspired by DeepTutor; AI-generated or curated |
| Test | **Vitest + cargo test** | Determinism via mocked PTY/Docker/LLM |

---

## Founders

LearnForge is built by two engineering educators with **1.27M+ combined enrollments** on Udemy and direct experience training engineers at scale.

### Gourav Shah — Product, DevOps domain
Founder, [School of DevOps](https://schoolofdevops.com) and [Agentix Garage](https://agentixgarage.com). 17+ years in DevOps, Cloud, and Platform Engineering. **270,000+ Udemy students** across 15+ courses. Trusted by Nasdaq, Volkswagen, NetApp. Author of *The Dawn of Agentic DevOps*.

### Vivian Aranha — AI, learning science
Data & AI Specialist at IBM, CEO of School of AI. Executive certification, MIT Sloan. Master's, GWU. **1,000,000+ Udemy enrollments**. 8+ years in AI/ML. Course author: *Mastering Agentic Design Patterns* (Udemy), *AI Engineer Complete Bootcamp* (Maven).

The product reflects what we've seen training real engineers: completion certificates don't matter. Demonstrated skill does. We're building the platform we wish existed when we were teaching.

---

## Roadmap

- **v1.0 — Open Source (Dec 2026)** — Adaptive loop closed, content richness via block taxonomy, hands-on labs (shipping), microlearning, certification, topic packs, web companion preview, full algorithm publication.
- **v1.1 — Corporate Foundation (Q1 2027)** — Multi-tenant cloud, cohorts, manager dashboards, multi-modal content (video + labs), gamification, Credly certs.
- **v2.0 — Enterprise (mid-2027)** — SSO/SAML, audit logging, SOC 2 controls, ROI analytics, SCORM/xAPI, custom topic packs, white-label.

See [`.planning/ROADMAP.md`](./.planning/ROADMAP.md) for phase-level detail and [`.planning/REQUIREMENTS.md`](./.planning/REQUIREMENTS.md) for the per-requirement traceability.

---

## License

- **Code** — MIT License ([`LICENSE`](LICENSE))
- **Documentation, algorithm specifications, learning science writeups** — Creative Commons Attribution 4.0 International ([`LICENSE-DOCS`](LICENSE-DOCS))
- **Third-party attributions** — [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) (DeepTutor patterns, Apache 2.0; portable-pty MIT; bollard Apache 2.0; gray_matter MIT; xterm.js MIT)

We chose MIT + CC BY 4.0 deliberately. Adaptive learning algorithms should be open, auditable, and available to all educators. They should not be locked behind proprietary systems or patents.

---

## Get involved

- **Star the repo** if any of this resonates — GitHub stars matter for nascent open-source education tooling.
- **File issues** with feedback, bugs, or topic-pack requests.
- **Contribute** — algorithm improvements, topic packs, accessibility work, AI provider integrations. See [`CONTRIBUTING.md`](CONTRIBUTING.md) (in progress).
- **Pilot LearnForge Cloud** — if you lead L&D at a GCC, software org, or platform team and want adaptive mastery-driven learning for your engineers, [open an issue](https://github.com/agentixgarage/learnforge/issues/new) tagged `pilot-interest` or reach the founders directly via the School of DevOps / School of AI sites linked above.

If your organization is spending six figures a year on L&D and cannot answer *"how many of our engineers can actually do X today?"* — we built this for you.

---

*LearnForge is an Agentix Garage and School of DevOps / School of AI project. Built in the open since March 2026.*
