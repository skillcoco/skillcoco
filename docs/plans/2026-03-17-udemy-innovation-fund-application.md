# Udemy Content Innovation Fund - Application Draft v2

> Track: New Content Proposal
> Type: Hybrid Approach
> Deadline: April 17, 2026
> Status: DRAFT v2 - Review and refine before submitting

---

## 9. Project Type

Hybrid approach

---

## 10. Project Title

Adaptive Mastery Courses: AI-Powered Technical Learning with Intelligent Mastery Tracking

---

## 11. Executive Summary
> In 3-5 sentences, describe what you want to build and what makes it meaningfully different.

We're creating a new adaptive course format for technical education that replaces linear video sequences with mastery-gated learning loops, powered by Udemy's AI features and proven learning science. Using Role Play for scenario-based technical assessment, Labs for hands-on practice with AI evaluation, and research-backed algorithms (Bayesian Knowledge Tracing for mastery measurement, SM-2 for spaced review scheduling), each learner follows a personalized path that adapts to their demonstrated understanding — not just their viewing history. We'll pilot this format with two high-demand courses — Agentic DevOps and Agentic AI Engineering — then publish a replicable Instructor Playbook so any technical instructor can adopt this format. We've already built and validated the adaptive learning algorithms in LearnForge, a working adaptive learning platform, and now want to bring this innovation to Udemy's 270 million learners.

---

## 12. What Are You Building, and Why?
> Up to 500 words

**What we're creating:**
An "Adaptive Mastery Course" — a new instructional format where course progression is driven by demonstrated mastery rather than video completion. We'll deliver two pilot courses in the two hottest enterprise skill domains — Agentic DevOps and Agentic AI Engineering — each structured as a skill tree rather than a linear sequence, where every learner's path adapts based on what they actually know.

**The learner challenge:**
Technical learners face three compounding problems. First, completion does not equal competence — Udemy's own 2026 Global Skills Report acknowledges that "completion rates tell a misleading story." Learners finish courses but cannot apply skills at work. Second, one-size-fits-all pacing wastes everyone's time — a senior engineer and a career-switcher get the same sequence of videos. Third, knowledge decays rapidly — without scientifically-timed review, research shows 80% of learned material is forgotten within 30 days. Current Udemy courses, however excellent their content, cannot address these because the format itself is linear and passive.

**What makes this meaningfully different:**
No course on Udemy currently combines adaptive assessment, mastery-gated progression, and spaced retention into a single learning experience. Our format introduces three innovations:

1. Mastery-gated progression: Learners must demonstrate understanding through Role Play scenarios and Lab exercises before unlocking the next skill branch. The system uses Bayesian Knowledge Tracing — the same algorithm used in intelligent tutoring systems at Carnegie Mellon — to probabilistically model what each learner truly knows, not what they've watched.

2. Adaptive pathways: Each course is structured as a directed acyclic graph (skill tree) with multiple valid paths. A sysadmin transitioning to DevOps takes a different route than a developer learning container orchestration. The AI assessment at course entry determines starting position and recommended path.

3. Spaced mastery loops: Using the SM-2 algorithm from spaced repetition research, learners receive timed review prompts that bring them back at scientifically optimal intervals, converting short-term understanding into lasting expertise.

**Why this format is well-suited:**
Both Agentic DevOps and Agentic AI Engineering are inherently non-linear and hands-on — you learn by building, not by watching. Role Play enables realistic scenarios ("Your AI agent is hallucinating in production — walk me through your diagnosis"). Labs enable actual practice. Together with mastery tracking, this creates a learning experience impossible to replicate with traditional video.

**How content is personalized:**
Every learner's experience differs based on three inputs: their assessed starting point (via entry Role Play), their demonstrated mastery on each skill branch (via BKT algorithm), and their retention patterns over time (via SM-2 scheduling). Learners who demonstrate mastery skip ahead. Those who struggle receive targeted reinforcement.

**What excites us:**
We've already built and validated these algorithms in LearnForge, an adaptive learning platform we developed with Bayesian Knowledge Tracing, SM-2 spaced repetition, and AI-powered DAG-based learning paths. Seeing learners progress through genuinely adaptive paths — where the course reshapes itself around their actual understanding — is transformative. This is what online education should feel like, and Udemy's platform features make it possible at scale.

---

## 13. Who Is This For?
> Describe your target learner (experience level, goals, job function/role, learning context).

**Agentic DevOps Course:** IT professionals and engineers (junior-to-mid level) building skills in AI-augmented infrastructure and operations. This includes sysadmins transitioning to DevOps who need Kubernetes, CI/CD, and now AI agent orchestration skills; developers bridging the gap between code and production; and junior DevOps engineers leveling up to handle agentic workflows in real infrastructure. Gartner predicts 40% of enterprise applications will embed task-specific AI agents by 2026 — these learners need to be ready.

**Agentic AI Engineering Course:** Software engineers and aspiring AI engineers (beginner-to-mid level) learning to design, build, and deploy AI agent systems. This includes developers moving into AI engineering roles; data scientists transitioning from notebooks to production agent systems; and technical professionals responding to the surging demand for AI Agent Engineers — a role where demand is outpacing supply by 10x with a 25% wage premium.

Both audiences are working professionals learning self-paced, often through Udemy Business with employer-supported training budgets. The adaptive format is especially valuable because their starting points vary dramatically — the current one-path-fits-all approach either bores experienced learners or overwhelms beginners.

---

## 14. Evidence of Demand
> Check all that apply.

- [x] Student feedback or requests
- [x] Industry trends/job demands
- [x] Gap in existing Udemy content
- [x] Educational research
- [x] Other - Write In: "Working adaptive learning platform (LearnForge) with validated algorithms already built"

---

## 15. What Will the Learning Experience Look Like?
> Up to 400 words. Structure/flow, interaction model, personalization, practice/reinforcement, mastery demonstration.

**Structure and flow:**
Each course is organized as a skill tree with 5-6 major branches (e.g., for Agentic DevOps: Foundations, Container Orchestration, AI Agent Integration, GitOps & Progressive Delivery, Observability & AIOps, Production Operations), each containing 3-5 skill modules. Rather than proceeding linearly, learners navigate branches based on prerequisites and goals. An AI-powered entry assessment via Role Play places each learner at their optimal starting point.

**Interaction model:**
Each skill module follows a "Mastery Loop" cycle:

Learn: Short, focused video instruction (5-10 min) with visual diagrams and reference material.
Practice: Hands-on Lab exercises where learners work in real environments with AI-evaluated outcomes.
Assess: Role Play scenarios testing applied understanding ("An AI agent is consuming excessive cluster resources and causing cascading failures — walk me through your response").
Verify: The Bayesian Knowledge Tracing algorithm calculates genuine understanding from practice and assessment results — not just whether the learner clicked "complete."

**Learner personalization:**
Three layers of personalization operate simultaneously. Path personalization: the entry assessment determines which branches to prioritize and which to skip. Depth personalization: learners who demonstrate mastery advance quickly, while those who struggle receive additional practice and alternative explanations. Timing personalization: the spaced review system (SM-2 algorithm) schedules return visits at intervals optimized for each learner's retention patterns.

**Practice and reinforcement:**
Every module includes hands-on Labs — not optional, but required for mastery verification. Role Play scenarios simulate realistic workplace situations, not abstract quiz questions. Failed mastery checks unlock targeted reinforcement content and additional practice rather than blocking progress. Spaced review prompts bring learners back days and weeks later to verify lasting retention.

**How learners demonstrate mastery:**
Mastery is demonstrated through Lab exercise outcomes (evaluated by AI for correctness and approach quality), Role Play scenario performance (evaluated against instructor-defined goals), and spaced review accuracy over time. The BKT algorithm synthesizes these signals into a probabilistic mastery score for each skill. Learners see their mastery profile as a visual skill map — showing genuine strengths and areas needing reinforcement — rather than a completion percentage. A learner "completes" the course when they've demonstrated verified mastery across core branches, with evidence of retention over time.

---

## 16. Expected Learning Outcomes
> 3-5 specific, measurable skills learners will be able to demonstrate upon completion.

**Agentic DevOps Course:**
1. Deploy, manage, and troubleshoot Kubernetes clusters with AI agent integration, verified through hands-on Labs and AI-evaluated troubleshooting scenarios
2. Design and implement agentic DevOps workflows where AI agents handle monitoring, incident response, and infrastructure optimization autonomously
3. Build production-grade CI/CD pipelines with GitOps practices and progressive delivery, incorporating AI-driven deployment decisions

**Agentic AI Engineering Course:**
4. Design and build multi-agent AI systems using modern frameworks (OpenAI Agents SDK, CrewAI, LangGraph), with mastery verified through hands-on projects
5. Deploy AI agents to production with proper observability, safety guardrails, and human-in-the-loop controls, demonstrated through realistic scenario assessments

**Both courses:**
All outcomes are verified at 30 and 90 days post-completion through spaced review assessments — measuring lasting mastery, not just end-of-course performance.

---

## 17. Will AI Be Used?

Yes - both

---

## 18. How Do You Intend to Use AI?

**How AI supports development:**
AI accelerates course creation in three ways. First, generating scenario variations for Role Play — from each base troubleshooting scenario, AI creates multiple variations with different failure modes, severity levels, and contextual details, dramatically expanding the practice surface without proportional instructor effort. Second, building adaptive content branches — when the mastery algorithm identifies a learner needs reinforcement, AI helps generate targeted supplementary explanations and exercises calibrated to their specific gap. Third, creating evaluation rubrics — AI helps design assessment criteria for Lab exercises, ensuring consistent mastery measurement across different learner approaches.

**How AI enhances the learner experience:**
AI is central to three learner-facing capabilities. Role Play delivers personalized scenario-based assessment where learners practice real technical conversations and troubleshooting — the AI adapts its responses based on the learner's answers, creating a dynamic dialogue rather than a static quiz. Lab exercises receive AI-powered evaluation that assesses not just correctness but approach quality, providing targeted feedback ("Your agent is running, but it lacks proper error boundaries — here's why that matters in production"). The mastery tracking system uses AI to synthesize signals from multiple activities into a coherent understanding of what each learner knows and to recommend personalized next steps.

**What learning benefit AI enables that traditional approaches cannot:**
True adaptive personalization at scale is impossible without AI. No human instructor can create unique learning paths for thousands of concurrent learners, evaluate their hands-on work in real time, and schedule scientifically-timed reviews for each individual. AI makes the "adaptive mastery loop" possible — the continuous cycle of assess, learn, practice, verify, and reinforce that responds to each learner's actual understanding. We've proven this works in LearnForge, our adaptive learning platform that implements these exact algorithms. This grant brings that validated intelligence to Udemy's platform and its 270 million learners.

---

## 19. How Will You Measure Successful Execution?
> List 2-3 metrics related to progress, completion, or delivery.

1. **Mastery retention rate:** Percentage of learners demonstrating verified skill mastery at 30 and 90 days post-completion through spaced review assessments (target: 60%+ retention at 90 days, versus industry standard ~20% for traditional video courses)

2. **Skill application rate:** Percentage of completers who report successfully applying learned skills at work within 60 days, measured via structured follow-up survey (target: 70%+)

3. **Engagement depth:** Ratio of active engagement time (Labs, Role Play, assessments) to passive consumption (video watching), targeting 60/40 active-to-passive split versus the typical 10/90 in standard courses

---

## 20. Expected Reach

**Current audience across channels:**
Combined Udemy student base: 370,000+ students across 20+ courses (Gourav: 270,000+; Vivian: 100,000+ with 1,000,000+ total Udemy enrollments). School of DevOps professional community. School of AI global community. Active LinkedIn presence (11,000+ followers combined). YouTube, Medium, and tech conference speaking.

**Awareness and enrollment plan if funded:**
Four channels: First, direct announcements to our existing 370,000+ Udemy students across both instructor profiles. Second, School of DevOps community activation — Gourav's established DevOps community includes working professionals at companies like Nasdaq, Volkswagen, and NetApp who are the exact target audience. Third, School of AI community and Vivian's AI engineering network for the Agentic AI course. Fourth, content marketing via LinkedIn, Medium, and YouTube covering the "adaptive mastery" methodology — Gourav's article "The Dawn of Agentic DevOps" already generated strong engagement in this space. Combined with organic Udemy discovery in two top-demand categories, we target 15,000+ enrollments across both courses within 6 months.

---

## 21. Estimated Timeline

3-5 months

---

## 22. Technical Feasibility
> Can this be built/executed within Udemy's current platform capabilities? If not, detail how you would approach building out a working prototype.

The core learning experience uses existing Udemy capabilities: Role Play for scenario-based assessments and adaptive conversations, Labs for hands-on practice, and structured course sections for content delivery. Course structure, video, quizzes, and all standard features work as-is.

What extends beyond current Udemy capabilities is the adaptive mastery tracking layer — the Bayesian Knowledge Tracing algorithm that models what each learner truly knows, and the SM-2 spaced repetition algorithm that schedules optimal review timing. These are not theoretical — we've already built and validated both algorithms in LearnForge, our adaptive learning platform. LearnForge implements BKT mastery tracking, SM-2 spaced repetition, AI-powered DAG-based learning paths, and on-demand content personalization as a fully functional system.

For the pilot courses, we'll implement mastery tracking as a companion web tool that supplements the Udemy course experience — learners complete activities on Udemy, and the companion tool provides the adaptive intelligence layer (mastery visualization, review scheduling, path recommendations). This follows the fund's stated openness to "independently building and testing a prototype" of capabilities Udemy doesn't currently offer.

This companion tool is designed as a proof-of-concept for native Udemy platform integration, with fully documented algorithms and clear integration patterns. The underlying technology already exists in LearnForge — we're adapting it to work with Udemy's platform, not building from scratch.

---

## 23. What Needs to Go Right?
> Most important factors that will determine whether this project succeeds.

1. **Role Play must work for technical assessment, not just soft skills.** Current Role Play adoption is overwhelmingly business communication and sales scenarios. We need to validate that it effectively supports technical troubleshooting dialogues with the same quality of adaptive conversation. Our LearnForge prototype confirms AI-driven technical assessment works — the key is mapping this to Udemy's Role Play engine. Early testing with the platform team would accelerate this.

2. **The mastery algorithms must produce genuinely actionable signals from Udemy learner interactions.** Bayesian Knowledge Tracing is well-proven in educational research and in our LearnForge implementation, but it needs to produce accurate mastery profiles from Udemy-specific inputs (Lab scores, Role Play evaluations, quiz results). We mitigate this through extensive beta testing with a 100+ learner cohort before public launch, with algorithm tuning based on real data.

3. **The format must be practically replicable by other instructors.** The innovation's largest impact comes from adoption across the instructor community. The Instructor Playbook must be clear enough that non-technical instructors can apply the format without building custom tools. We'll validate this by having 3-5 external instructors attempt to apply the format during development.

---

## 24. Your Qualifications
> Why are you well positioned to execute this innovation?

**Gourav Shah** brings 17+ years in DevOps, Cloud, and Platform Engineering. As founder of School of DevOps and Agentix Garage, he's trained engineers at Nasdaq, Volkswagen, NetApp, and numerous other enterprises. With 270,000+ Udemy students across 15+ courses and 7+ years as a premium Udemy instructor, he deeply understands both the technical domain and the Udemy learner journey. His article "The Dawn of Agentic DevOps" captured the industry shift he'll be teaching. He holds Linux Foundation certifications and has built LearnForge — the adaptive learning platform whose algorithms power this proposal. He knows exactly where learners get stuck, which concepts need reinforcement, and how to design non-linear skill progressions from observing 270,000+ students.

**Vivian Aranha** is a Data & AI Specialist at IBM and CEO of School of AI, with nearly 20 years in the tech industry, 8+ years specializing in AI, ML, and deep learning, and 1,000,000+ Udemy enrollments. He holds an Executive Certification from MIT Sloan School of Management and a Master's from The George Washington University. His Udemy course "Mastering Agentic Design Patterns" and his Maven "AI Engineer Complete Bootcamp" demonstrate deep expertise in exactly the agentic AI domain this proposal covers. He brings the AI architecture and adaptive system expertise, combined with proven ability to teach complex concepts at massive scale.

**Together**, we combine deep domain expertise in both piloted topics, AI engineering capability, massive Udemy platform experience (370,000+ students combined), and a working adaptive learning platform (LearnForge) that already implements the exact algorithms this proposal uses. We've already built, tested, and validated Bayesian Knowledge Tracing, SM-2 spaced repetition, and AI-powered adaptive path generation. This isn't a theoretical proposal — the learning science is implemented, and we're bringing it to Udemy.

---

## 25. Total Grant Amount Requested

$100,000

---

## 26. Budget Summary
> Concise breakdown: instructor time, production, tools/software, external resources.

**Instructor time (2 instructors, ~500 hours each over 4 months): $50,000**
- Course architecture and adaptive skill tree design (2 courses)
- Content creation — scripts, diagrams, 60+ Role Play scenarios with variations
- Lab exercise design, validation, and environment configuration
- Recording, review, and iteration cycles
- Beta testing coordination and mastery algorithm tuning
- Instructor Playbook authoring and external validation

**Production (video, editing, graphics, audio): $12,000**
- Professional video recording and editing for 2 full courses
- Skill tree visualizations, architecture diagrams, mastery map designs
- Thumbnails, promotional materials, and Playbook design

**Tools, software, and infrastructure: $18,000**
- Cloud infrastructure for Kubernetes and AI Lab environments
- Companion mastery tracking tool — adapting LearnForge's algorithms for Udemy integration
- AI API costs for scenario generation, evaluation, and testing during development
- Development tooling, hosting, and CI/CD for the companion prototype

**Beta testing and research: $12,000**
- Beta cohort recruitment and coordination (100+ learners per course)
- Data collection infrastructure and analysis tooling
- 30/60/90-day follow-up survey infrastructure and execution
- External instructor Playbook validation (3-5 instructors)

**Playbook and documentation: $8,000**
- Instructor Playbook production — format specification, implementation guide, case study
- Algorithm documentation with clear integration patterns for Udemy platform consideration
- Outcome report with comparative data (adaptive vs. traditional format)

---

## 27. Why Is This Funding Important?
> How the grant enables or expands this innovation.

We've already invested significant time and resources building LearnForge — an adaptive learning platform that implements the exact mastery tracking, spaced repetition, and adaptive path algorithms this proposal brings to Udemy. The core technology is proven and working.

This grant enables us to go full-time on adapting that technology for Udemy's platform and creating two flagship courses that demonstrate its impact. Specifically:

First, adapting LearnForge's adaptive engine for Udemy. Building the companion tool that bridges our validated algorithms (BKT, SM-2, adaptive DAG paths) with Udemy's Role Play, Labs, and course infrastructure requires dedicated development — translating a standalone system into something that enhances Udemy's platform.

Second, creating two complete adaptive courses in the hottest enterprise skill domains. Each course requires 3-4x the design work of a traditional course — non-linear skill trees, 60+ Role Play scenarios, mastery-gated Lab exercises, and spaced review architecture. This is a full-time commitment for both instructors.

Third, rigorous outcome measurement. The value proposition rests on measurable mastery improvement. Running proper betas with 100+ learners per course, 90-day longitudinal tracking, and comparative analysis requires dedicated resources.

Fourth, the Instructor Playbook. The innovation's scale depends on other instructors adopting it. Professional documentation, external validation, and a clear integration specification transforms two courses into a format that could redefine technical education across Udemy's entire instructor community.

This grant transforms proven technology into a platform-wide innovation.

---

## 28. Optional Materials (Links)

[See companion document: docs/plans/2026-03-17-udemy-fund-optional-materials.md]

Suggested links to include:
- LearnForge prototype demo / repository
- LearnForge design documentation and algorithm specifications
- Gourav's "The Dawn of Agentic DevOps" article
- Vivian's "Mastering Agentic Design Patterns" Udemy course
- School of DevOps website / credentials

---

## 29. Optional Materials (Upload)

[See companion document: docs/plans/2026-03-17-udemy-fund-optional-materials.md]

Suggested uploads:
- One-page "Adaptive Mastery Format" visual overview (to be created)
- LearnForge prototype screenshots showing mastery tracking
- Sample Role Play scenario scripts for Agentic DevOps
- Adaptive path (DAG) diagram showing skill tree structure
- Plain-language summary of BKT and SM-2 algorithms

---

## Selection Choices Summary

| Question | Selection |
|----------|-----------|
| Q9. Project Type | Hybrid approach |
| Q14. Evidence of Demand | Student feedback, Industry trends, Gap in existing content, Educational research, Other: "Working adaptive learning platform (LearnForge)" |
| Q17. Will AI Be Used? | Yes - both |
| Q21. Estimated Timeline | 3-5 months |

---

## Strategic Notes (DO NOT INCLUDE IN APPLICATION)

### LearnForge Positioning Strategy
- LearnForge is mentioned as "a working adaptive learning platform" — established technology, not a prototype sketch
- Demonstrates we have real IP (algorithms, architecture, implementation) not just ideas
- Framed as: "we want to bring this TO Udemy" — implies it exists independently and could go elsewhere
- The companion tool is described as "adapting LearnForge's algorithms for Udemy integration" — signals that Udemy gets access to something valuable
- The grant essentially buys Udemy first access to this innovation on their platform
- If they see competitive potential, the conversation naturally evolves toward deeper partnership/acquisition

### Budget Rationale
- $100K = 4% of $2.5M fund — substantial but not greedy
- Justified by scope: 2 courses + companion tool + playbook + research
- Signals this is serious, full-time work requiring both instructors to deprioritize other income
- The companion tool development alone justifies the premium over standard course grants

### Topic Selection Rationale
- Agentic DevOps: Gartner-predicted trend, Gourav's deep expertise, massive enterprise demand
- Agentic AI Engineering: 10x supply/demand imbalance, 25% wage premium, Vivian's expertise
- Both are #1 enterprise priorities for Udemy Business (where the revenue is)
- Both naturally showcase the adaptive format (non-linear, hands-on, mastery-critical)
