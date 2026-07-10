# Workshop / Team Environment Requirements

**One-pager for facilitators running a LearnForge workshop or team pilot.**
Read this before the session — every item below is a pre-flight check, not
a runtime surprise.

LearnForge pairs with two Phase 18 deliverables for workshop/team pilots:
signed **skill-report export** (per-learner, verifiable evidence of
knowledge + practical mastery) and the **`scripts/skill-report-aggregate.py`**
cohort-summary tool, which verifies every learner's exported report and
prints a group-level distribution across the cohort. This document covers
the environment those exports and labs run in — not the tooling itself.

## 1. Docker Desktop availability

Hands-on labs run best inside a Docker container: full filesystem/network
isolation from the host, a clean image per lab, no leftover state between
learners or sessions. Before a workshop, confirm Docker Desktop (or an
equivalent OCI runtime) is installed and running on every participant
machine.

- Locked-down corporate laptops are the most common failure point —
  Docker Desktop requires admin rights to install on most platforms.
  Confirm installation ahead of the session, not on the day.
- If Docker cannot run, LearnForge's runtime setting can be switched to
  Auto-detect or Host-shell — see the fallback section below — but plan
  for it in advance rather than discovering it mid-lab.

## 2. Host-shell fallback implications

When Docker is unavailable, LearnForge falls back to running lab steps
directly in a host shell (a real PTY on the learner's own machine, not a
container). This keeps labs functional on locked-down machines, but the
isolation and safety guarantees are meaningfully weaker:

- Commands execute with the learner's own OS user permissions — no
  container boundary contains a mistaken `rm -rf` or a runaway process.
- State can persist across lab attempts (files, installed packages,
  environment variables) in a way a fresh container never would, which
  can also make lab evaluation less reliable session-to-session.
- Some lab content explicitly requires Docker (marked `requires_docker`
  in the lab spec) and will surface a notice offering to switch back to
  Auto-detect for that lab if the learner is pinned to host-shell only.

Facilitators running a workshop on mixed hardware (some laptops with
Docker, some without) should expect this split and set expectations with
participants beforehand — host-shell learners are still fully able to
complete labs, just with a different isolation trade-off.

## 3. Network / proxy needs

- **AI-judged labs and AI-assisted content generation** need outbound
  reachability to the configured AI provider's API endpoint. Corporate
  proxies that intercept TLS or block unfamiliar domains are the most
  common cause of "AI features silently not working" in a workshop
  setting — confirm proxy allowlists ahead of time if participants are on
  a managed corporate network.
- **Skill-report submission to an org report server** (if configured via
  Settings) is fire-and-forget: a failed or offline submission never
  blocks the learner and never blocks file export. Treat server
  submission as a nice-to-have during a workshop, not a dependency;
  retry/queueing is best-effort, and signed reports are still exported as
  files even if the network is fully unavailable.
- **File export always works fully offline.** Signed skill reports (JSON
  + PDF) are generated and written entirely on-device — no network call is
  required to produce or verify them. An air-gapped or fully offline
  workshop can still produce verifiable, signed evidence for every
  learner; only the AI-generation and AI-judged-lab features require
  connectivity.

## 4. BYO-key decision (facilitator pre-flight)

LearnForge's AI features (path generation, AI-judged lab steps, content
generation) require an AI provider credential per learner install — there
is no shared server-side key. Before the session, the facilitator must
decide and communicate one of:

- **Bring-your-own-key (BYOK):** each participant supplies their own AI
  provider credential (API key or OAuth login) in Settings before the
  workshop starts.
- **Provided keys:** the facilitator distributes a workshop-scoped
  credential (e.g. a short-lived or budget-capped key) to participants
  ahead of time, with clear guidance on where to enter it in Settings.
- **No AI access:** the workshop proceeds without AI-generated content or
  AI-judged lab steps; participants use pre-built topic packs and
  command-based (non-AI) lab checks only. Skill-report export and
  aggregation are unaffected — they contain no AI dependency of their own.

Whichever option is chosen, communicate it to participants before the
session so nobody arrives without a working AI credential when the
workshop plan assumes one.

## 5. Pre-flight checklist

Run through this list with (or send to) participants before the session:

- [ ] LearnForge installed and opens without errors.
- [ ] Docker Desktop installed and running (or: participant has been told
      they will run labs in host-shell fallback mode).
- [ ] Network/proxy access confirmed to the AI provider's API endpoint (if
      AI features are in scope for the session).
- [ ] AI provider credential decision made and communicated (BYOK,
      provided key, or no-AI mode) and entered in Settings if applicable.
- [ ] A quick smoke test completed: open one lab, run one command/step,
      confirm evaluation feedback appears.
- [ ] For team pilots collecting cohort evidence: confirm the facilitator
      has the signing public key(s) needed to run
      `scripts/skill-report-aggregate.py` against exported reports after
      the session, and that participants know how to export their signed
      skill report (JSON + PDF) at the end.

Facilitators who complete this checklist before the session avoid the
most common workshop-day failure modes: a laptop that can't run Docker,
a proxy silently blocking AI calls, and a missing or expired AI credential
discovered only once learners are already mid-session.
