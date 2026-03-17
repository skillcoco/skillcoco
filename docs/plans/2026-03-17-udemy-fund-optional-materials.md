# Optional Materials for Udemy Innovation Fund Application

## 1. One-Page Format Overview (for upload as PDF)

---

### ADAPTIVE MASTERY COURSES
#### A New Instructional Format for Technical Education on Udemy

**The Problem:** Completion =/= Competence
- Udemy's 2026 Global Skills Report: "Completion rates tell a misleading story"
- 80% of learned material forgotten within 30 days without optimized review
- One-size-fits-all pacing wastes time for both beginners and experienced learners

**The Solution: The Mastery Loop**

```
                    ┌─────────────┐
                    │  AI Entry   │
                    │ Assessment  │
                    │ (Role Play) │
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
              ┌─────┤  Skill Tree  ├─────┐
              │     │  Navigation  │     │
              │     └──────┬──────┘     │
              │            │            │
        ┌─────▼───┐  ┌────▼────┐  ┌───▼─────┐
        │Branch A │  │Branch B │  │Branch C │
        │(Skip if │  │(Start   │  │(Unlock  │
        │mastered)│  │ here)   │  │  later) │
        └─────────┘  └────┬────┘  └─────────┘
                          │
                 ┌────────▼────────┐
                 │   MASTERY LOOP  │
                 │                 │
                 │ 1. LEARN        │
                 │    Short video  │
                 │    + diagrams   │
                 │                 │
                 │ 2. PRACTICE     │
                 │    Hands-on Lab │
                 │    AI-evaluated │
                 │                 │
                 │ 3. ASSESS       │
                 │    Role Play    │
                 │    scenario     │
                 │                 │
                 │ 4. VERIFY       │
                 │    BKT mastery  │
                 │    calculation  │
                 └────────┬────────┘
                          │
                ┌─────────▼─────────┐
                │  Mastery >= 70%?  │
                ├─────────┬─────────┤
                │  YES    │   NO    │
                │         │         │
           ┌────▼───┐ ┌──▼────────┐
           │Unlock  │ │Reinforce: │
           │next    │ │Alt. expl. │
           │branch  │ │More labs  │
           │+ Create│ │Retry loop │
           │SR cards│ └───────────┘
           └────┬───┘
                │
        ┌───────▼───────┐
        │ SPACED REVIEW │
        │  (SM-2 algo)  │
        │               │
        │ Day 1 → Day 6 │
        │ → Day 15 → ...│
        │               │
        │ Long-term     │
        │ retention     │
        └───────────────┘
```

**Three Layers of Personalization:**

| Layer | How It Works | Powered By |
|-------|-------------|------------|
| Path | Entry assessment determines starting branch and route | Role Play + AI |
| Depth | Mastery level determines pace — skip or reinforce | BKT Algorithm |
| Timing | Review schedule adapts to individual retention | SM-2 Algorithm |

**Pilot Courses:**

| Course | Domain | Lead Instructor | Enterprise Demand |
|--------|--------|----------------|-------------------|
| Agentic DevOps | K8s + AI Agents for Infrastructure | Gourav Shah (270K+ students) | Gartner: 40% enterprise apps will embed AI agents by 2026 |
| Agentic AI Engineering | Building & Deploying AI Agent Systems | Vivian Aranha (1M+ enrollments) | 10x supply/demand gap, 25% wage premium |

**Already Validated:**
LearnForge — our adaptive learning platform — implements BKT, SM-2, and AI-powered adaptive paths as working software. The algorithms are proven. This grant brings them to Udemy.

---

## 2. Sample Role Play Scenarios

### Agentic DevOps - Scenario 1: Production Incident Response

**Scenario Title:** "The Cascading Failure"

**Setup for learner:**
You are a DevOps engineer on the platform team at a mid-size SaaS company. It's Tuesday morning and the monitoring system just fired a P1 alert: three microservices in the payments cluster are crash-looping, customer-facing APIs are returning 503 errors, and the AI monitoring agent has flagged an unusual resource consumption pattern that started 20 minutes ago.

Your team lead has just joined the call and is asking you to lead the investigation.

**AI Character:** Team Lead (experienced, calm, asking probing questions)

**First line:** "Alright, I can see the dashboards lighting up. The AI agent flagged something about resource limits being hit. What's your first move — where do you start looking?"

**Evaluation Goals (up to 5):**
1. Demonstrates systematic troubleshooting approach (doesn't jump to conclusions)
2. Uses appropriate kubectl commands to gather information (logs, describe, top)
3. Identifies the root cause (resource limit misconfiguration after recent deployment)
4. Proposes both immediate fix and preventive measures
5. References the AI monitoring agent's data appropriately in the diagnosis

---

### Agentic DevOps - Scenario 2: Agent Architecture Decision

**Scenario Title:** "Designing the AIOps Pipeline"

**Setup for learner:**
Your company is adopting agentic DevOps practices. The CTO has asked you to propose an architecture for AI agents that will handle automated incident detection, root cause analysis, and remediation for the Kubernetes infrastructure. You're presenting your design to the senior engineering team.

**AI Character:** Senior Staff Engineer (skeptical, detail-oriented, concerned about safety)

**First line:** "I've seen the proposal outline. Before we go further — what's your approach to ensuring these agents don't make things worse? Last thing we need is an AI agent auto-scaling us into a $50K cloud bill."

**Evaluation Goals:**
1. Addresses safety guardrails and human-in-the-loop controls
2. Proposes appropriate agent boundaries (what agents can vs. cannot do autonomously)
3. Demonstrates understanding of observability requirements for agent actions
4. Considers rollback mechanisms and blast radius limitation
5. Shows practical knowledge of available AIOps frameworks and tools

---

### Agentic AI Engineering - Scenario 1: Multi-Agent System Debug

**Scenario Title:** "The Hallucinating Agent"

**Setup for learner:**
You built a multi-agent customer support system using CrewAI. The system has been in production for two weeks and performing well, but today QA flagged an issue: the research agent is occasionally returning fabricated product specifications, and the response agent is confidently presenting them to customers. Three customers have already received incorrect technical specs.

**AI Character:** Product Manager (non-technical, concerned about customer impact, needs clear explanation)

**First line:** "I just got off a call with an angry customer who ordered based on specs your AI gave them — specs that don't match any product we sell. Can you explain what happened and how we fix this?"

**Evaluation Goals:**
1. Explains hallucination in accessible terms for non-technical stakeholder
2. Identifies likely root cause (retrieval failure, missing grounding, inadequate validation)
3. Proposes immediate mitigation (human review, confidence thresholds)
4. Outlines systematic fix (RAG improvements, fact-checking agent, output validation)
5. Addresses process improvement to prevent recurrence

---

## 3. Adaptive Skill Tree Diagram (for upload)

### Agentic DevOps Course — Skill Tree Structure

```
                        ┌──────────────────┐
                        │  ENTRY ASSESSMENT │
                        │   (Role Play)    │
                        │                  │
                        │ Determines:      │
                        │ - Starting level │
                        │ - Recommended    │
                        │   path           │
                        └────────┬─────────┘
                                 │
              ┌──────────────────┼──────────────────┐
              │                  │                  │
     ┌────────▼───────┐ ┌──────▼───────┐ ┌───────▼────────┐
     │  FOUNDATIONS   │ │  CONTAINERS  │ │  (Skip if      │
     │               │ │              │ │   assessed     │
     │ - Linux/CLI   │ │ - Docker     │ │   as mid+)     │
     │ - Networking  │ │ - Images     │ │                │
     │ - YAML/Config │ │ - Compose    │ │                │
     └────────┬───────┘ └──────┬───────┘ └───────┬────────┘
              │                │                  │
              └────────┬───────┘                  │
                       │                          │
              ┌────────▼──────────┐               │
              │    KUBERNETES     │◄──────────────┘
              │    CORE           │
              │                   │
              │ - Architecture    │
              │ - Pods & Deploys  │
              │ - Services        │
              │ - ConfigMaps      │
              └────────┬──────────┘
                       │
         ┌─────────────┼─────────────┐
         │             │             │
┌────────▼───────┐ ┌──▼──────────┐ ┌▼──────────────┐
│  NETWORKING    │ │  STORAGE &  │ │  SECURITY &   │
│  & INGRESS    │ │  STATE      │ │  RBAC         │
│               │ │             │ │               │
│ - CNI         │ │ - PV/PVC   │ │ - RBAC        │
│ - Ingress     │ │ - StateSets│ │ - NetworkPol  │
│ - Service     │ │ - Operators│ │ - Secrets     │
│   Mesh        │ │            │ │ - Pod Sec     │
└────────┬───────┘ └──────┬─────┘ └───────┬───────┘
         │                │               │
         └────────┬───────┘               │
                  │                       │
         ┌────────▼──────────┐            │
         │  CI/CD & GITOPS   │◄───────────┘
         │                   │
         │ - GitHub Actions  │
         │ - ArgoCD / Flux   │
         │ - Progressive     │
         │   Delivery        │
         └────────┬──────────┘
                  │
         ┌────────▼──────────┐
         │  AGENTIC DEVOPS   │
         │  (Advanced)       │
         │                   │
         │ - AI Monitoring   │
         │   Agents          │
         │ - Auto-Remediation│
         │ - AIOps Pipelines │
         │ - Agent Safety &  │
         │   Guardrails      │
         └────────┬──────────┘
                  │
         ┌────────▼──────────┐
         │  PRODUCTION OPS   │
         │  (Capstone)       │
         │                   │
         │ - Multi-cluster   │
         │ - Scaling & HA    │
         │ - Disaster        │
         │   Recovery        │
         │ - Cost Optim.     │
         └──────────────────┘

Legend:
──── Prerequisite (must complete)
- - - Recommended (helpful but optional)
Each module = Mastery Loop (Learn → Practice → Assess → Verify)
```

---

## 4. BKT & SM-2 Algorithm Summary (Plain Language)

### Bayesian Knowledge Tracing (BKT) — How We Measure Real Mastery

**What it does:**
Instead of asking "did they complete the video?", BKT asks "what is the probability this learner actually understands this concept?" It maintains a running probability of mastery for each skill, updated every time the learner practices.

**How it works (simplified):**
- Learner gets a practice question RIGHT → mastery probability goes UP (but accounts for lucky guesses)
- Learner gets a practice question WRONG → mastery probability goes DOWN (but accounts for careless mistakes)
- After each interaction, the model updates using Bayes' theorem with four parameters:
  - Initial knowledge probability (how likely they knew it before the course)
  - Learning rate (how quickly they pick up new concepts per practice)
  - Guess rate (how often someone gets it right by luck)
  - Slip rate (how often someone who knows it gets it wrong by accident)

**Why it matters for Udemy:**
Traditional courses measure: "Did they click through all the videos?" (completion)
BKT measures: "Based on their Lab performance, Role Play responses, and quiz answers, there's a 73% probability this learner has mastered Kubernetes networking." (actual mastery)

This is the difference between a participation trophy and a verified skill.

**Research foundation:** BKT was developed at Carnegie Mellon University and is used in intelligent tutoring systems worldwide. It's the standard algorithm for mastery modeling in educational technology research.

---

### SM-2 Spaced Repetition — How We Make Knowledge Stick

**What it does:**
SM-2 calculates the optimal time to review a concept so it transfers from short-term to long-term memory. Instead of cramming everything at once, it spaces reviews at increasing intervals based on how well the learner remembers.

**How it works (simplified):**
- First review: 1 day after learning
- Second review: 6 days later
- Each subsequent review: previous interval multiplied by an "ease factor"
- If the learner remembers easily → interval grows faster (they know it well)
- If the learner struggles → interval resets to 1 day (they need more practice)

**Example journey for "Kubernetes Pod Scheduling":**
- Day 0: Learner completes the module with 75% mastery
- Day 1: Review prompt — learner answers correctly, rated "Good" → next review in 6 days
- Day 7: Review prompt — learner answers correctly, rated "Easy" → next review in 15 days
- Day 22: Review prompt — learner struggles, rated "Hard" → next review in 3 days
- Day 25: Review prompt — learner answers correctly → next review in 12 days
- ...continues, with intervals growing as knowledge solidifies

**Why it matters for Udemy:**
Research shows that without spaced review, learners forget ~80% of material within 30 days. With SM-2 scheduling, retention at 90 days can exceed 60%. This is the difference between a course that felt good in the moment and skills that last.

**Research foundation:** SM-2 was developed by Piotr Wozniak and has been validated in decades of spaced repetition research. It's the algorithm behind SuperMemo and has influenced Anki and every modern flashcard system.

---

## 5. LearnForge — Existing Technology Summary

### What Is LearnForge?

LearnForge is an adaptive learning platform we've built that implements the exact algorithms proposed in this application:

**Implemented and validated:**
- Bayesian Knowledge Tracing for real-time mastery modeling
- SM-2 spaced repetition with configurable parameters
- DAG-based adaptive learning paths (directed acyclic graph skill trees)
- AI-powered content generation personalized to learner state
- AI-driven exercise creation and evaluation
- Socratic AI assessment for entry-level placement
- Multi-provider AI integration (works with Claude, OpenAI, Gemini, Ollama)

**Technology:**
- Rust backend with embedded vector intelligence
- BKT and SM-2 algorithms implemented and unit-tested
- SQLite with WAL mode for concurrent access
- React/TypeScript frontend with real-time mastery visualization

**Why this matters for the proposal:**
This is not a theoretical application. Every algorithm, every adaptive mechanism, and every mastery tracking capability described in this proposal exists as working, tested software. The grant enables us to bring this proven technology to Udemy's platform and demonstrate it through two flagship courses.

---

## Materials Checklist

- [ ] Convert Section 1 (One-Page Overview) to designed PDF
- [ ] Get LearnForge screenshots (mastery tracking UI, DAG visualization, assessment flow)
- [ ] Record 2-minute demo video of LearnForge adaptive flow (optional but powerful)
- [ ] Convert Section 3 (Skill Tree) to professional diagram
- [ ] Convert Section 4 (Algorithm Summary) to designed one-pager
- [ ] Gather links: Gourav's LinkedIn article, Vivian's Udemy courses, School of DevOps
