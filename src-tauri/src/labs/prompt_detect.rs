//! # labs::prompt_detect — OSC 133 + heuristic prompt-boundary detection
//! (Phase 03.1, Wave 2)
//!
//! Streaming byte-level FSM. Recognises OSC 133 A/B/C/D sequences:
//!
//!   ESC ] 1 3 3 ; A BEL              -- prompt start
//!   ESC ] 1 3 3 ; B BEL              -- prompt end (command region begins)
//!   ESC ] 1 3 3 ; C BEL              -- command executed
//!   ESC ] 1 3 3 ; D ; <code> BEL     -- command ended with exit code
//!
//! Both BEL (0x07) and ST (ESC \\, bytes 0x1B 0x5C) terminate the OSC.
//!
//! When OSC 133 is absent, a heuristic fallback fires after
//! `HEURISTIC_TIMEOUT_SECS` of silence: if the buffer ends with a typical
//! prompt trailer (`$ `, `# `, `> `) emit `CommandEnd { exit_code: None }`.
//! Tests drive the timeout via `tick(seconds)` rather than wall-clock so
//! they're deterministic and independent of `tokio::time::pause`.

#![allow(clippy::needless_range_loop)]

use super::LabError;

/// Heuristic fallback delay — emit CommandEnd { exit_code: None } when no
/// OSC 133 D has arrived within this window of silence and the buffer ends
/// in a prompt-style trailer.
pub const HEURISTIC_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptEvent {
    PromptStart,
    CommandStart,
    OutputStart,
    CommandEnd { exit_code: Option<i32> },
}

/// Internal parser state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseState {
    /// Normal byte-stream — looking for ESC.
    Normal,
    /// Saw `ESC` — looking for `]`.
    SawEsc,
    /// Inside an OSC payload — accumulating until BEL or ST. The `is_133`
    /// flag tracks whether the prefix is `133;`.
    InOsc { is_133: bool, esc_pending: bool },
}

/// Streaming detector.
pub struct PromptDetector {
    state: ParseState,
    osc_buffer: Vec<u8>,
    /// Total elapsed silence (in seconds) since the last visible byte.
    /// Driven by `tick()`; reset to 0 on each `feed()` call that consumed
    /// any non-OSC bytes (i.e. the user is still seeing output).
    silence_secs: u64,
    /// Trailing bytes of non-OSC output (capped) — used by the heuristic
    /// fallback to detect `$ ` / `# ` / `> ` trailers.
    tail: Vec<u8>,
    /// Set once the heuristic CommandEnd has been emitted so we don't
    /// re-emit it on subsequent ticks.
    heuristic_emitted: bool,
    /// Set once we've seen at least one OSC 133 sequence — disables the
    /// heuristic permanently (the shell is OSC-133 aware).
    osc133_seen: bool,
}

const TAIL_CAP: usize = 16;

impl PromptDetector {
    pub fn new() -> Self {
        Self {
            state: ParseState::Normal,
            osc_buffer: Vec::new(),
            silence_secs: 0,
            tail: Vec::new(),
            heuristic_emitted: false,
            osc133_seen: false,
        }
    }

    /// Feed bytes into the detector and drain emitted events. Strips
    /// OSC sequences from the buffer; non-OSC bytes accumulate into the
    /// tail buffer for heuristic-fallback inspection.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<PromptEvent>, LabError> {
        let mut events = Vec::new();
        let mut consumed_visible = false;

        for &b in bytes {
            match self.state {
                ParseState::Normal => {
                    if b == 0x1B {
                        self.state = ParseState::SawEsc;
                    } else {
                        self.push_tail(b);
                        consumed_visible = true;
                    }
                }
                ParseState::SawEsc => {
                    if b == b']' {
                        // Start of an OSC. We don't yet know whether it's 133;
                        // the buffer accumulates until we hit BEL / ST.
                        self.osc_buffer.clear();
                        self.state = ParseState::InOsc {
                            is_133: false,
                            esc_pending: false,
                        };
                    } else {
                        // Some other CSI / escape — emit ESC + byte to the
                        // tail and return to Normal. ANSI cursor / color
                        // codes don't matter for prompt detection.
                        self.push_tail(0x1B);
                        self.push_tail(b);
                        self.state = ParseState::Normal;
                        consumed_visible = true;
                    }
                }
                ParseState::InOsc {
                    mut is_133,
                    mut esc_pending,
                } => {
                    // Detect the `133;` prefix on the fly.
                    if !is_133 && self.osc_buffer.len() < 4 {
                        self.osc_buffer.push(b);
                        if self.osc_buffer.starts_with(b"133;") {
                            is_133 = true;
                        } else if self.osc_buffer.len() == 4
                            && self.osc_buffer != b"133;"
                        {
                            // Non-133 OSC. We still need to drain until the
                            // terminator, but we can drop the buffer.
                            is_133 = false;
                        }
                        self.state = ParseState::InOsc {
                            is_133,
                            esc_pending,
                        };
                        continue;
                    }

                    // Already past prefix detection; track the terminator.
                    if esc_pending {
                        // We saw ESC inside the OSC — only ESC \\ ends it.
                        if b == b'\\' {
                            // ST terminator — emit events if 133.
                            if is_133 {
                                events.extend(parse_133_payload(&self.osc_buffer));
                                self.osc133_seen = true;
                            }
                            self.osc_buffer.clear();
                            self.state = ParseState::Normal;
                        } else {
                            // ESC <other> — abort and re-process this byte.
                            // For safety: drop OSC accumulation and treat as
                            // raw bytes.
                            self.osc_buffer.clear();
                            self.state = ParseState::Normal;
                            self.push_tail(0x1B);
                            self.push_tail(b);
                            consumed_visible = true;
                        }
                        continue;
                    }

                    if b == 0x07 {
                        // BEL terminator.
                        if is_133 {
                            events.extend(parse_133_payload(&self.osc_buffer));
                            self.osc133_seen = true;
                        }
                        self.osc_buffer.clear();
                        self.state = ParseState::Normal;
                    } else if b == 0x1B {
                        // Possible ST start — wait for backslash.
                        esc_pending = true;
                        self.state = ParseState::InOsc { is_133, esc_pending };
                    } else {
                        if is_133 {
                            self.osc_buffer.push(b);
                        }
                        self.state = ParseState::InOsc { is_133, esc_pending };
                    }
                }
            }
        }

        if consumed_visible {
            self.silence_secs = 0;
        }

        Ok(events)
    }

    /// Indicate that `seconds` of silence have elapsed (heuristic fallback).
    /// Returns events emitted by the fallback, if any.
    pub fn tick(&mut self, seconds: u64) -> Result<Vec<PromptEvent>, LabError> {
        self.silence_secs = self.silence_secs.saturating_add(seconds);
        let mut events = Vec::new();

        // Heuristic only runs when OSC 133 hasn't been seen — once the shell
        // is OSC-133-aware we trust the markers exclusively.
        if self.osc133_seen || self.heuristic_emitted {
            return Ok(events);
        }
        if self.silence_secs < HEURISTIC_TIMEOUT_SECS {
            return Ok(events);
        }
        if ends_with_prompt_trailer(&self.tail) {
            events.push(PromptEvent::CommandEnd { exit_code: None });
            self.heuristic_emitted = true;
        }
        Ok(events)
    }

    fn push_tail(&mut self, b: u8) {
        self.tail.push(b);
        if self.tail.len() > TAIL_CAP {
            let drop = self.tail.len() - TAIL_CAP;
            self.tail.drain(..drop);
        }
    }
}

impl Default for PromptDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse the body of an OSC 133 sequence (without the leading `133;` prefix
/// or terminator) into one or more PromptEvents.
fn parse_133_payload(buffer: &[u8]) -> Vec<PromptEvent> {
    // buffer looks like b"133;A" or b"133;D;0" — strip the prefix.
    let body = if buffer.starts_with(b"133;") {
        &buffer[4..]
    } else {
        buffer
    };
    if body.is_empty() {
        return vec![];
    }
    match body[0] {
        b'A' => vec![PromptEvent::PromptStart],
        b'B' => vec![PromptEvent::CommandStart],
        b'C' => vec![PromptEvent::OutputStart],
        b'D' => {
            let exit = if body.len() > 2 && body[1] == b';' {
                std::str::from_utf8(&body[2..])
                    .ok()
                    .and_then(|s| s.trim().parse::<i32>().ok())
            } else {
                None
            };
            vec![PromptEvent::CommandEnd { exit_code: exit }]
        }
        _ => vec![],
    }
}

/// Heuristic check — does the trailing buffer end with `$ ` / `# ` / `> `?
fn ends_with_prompt_trailer(tail: &[u8]) -> bool {
    let s = match std::str::from_utf8(tail) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let trimmed = s.trim_end_matches('\n');
    trimmed.ends_with("$ ")
        || trimmed.ends_with("# ")
        || trimmed.ends_with("> ")
        || trimmed.ends_with("$")
        || trimmed.ends_with("#")
}

#[cfg(test)]
mod tests {
    use super::*;

    const KUBECTL_GET_PODS: &[u8] =
        include_bytes!("../../tests/fixtures/labs/transcripts/kubectl-get-pods.bytes");
    const EXIT_ZERO: &[u8] =
        include_bytes!("../../tests/fixtures/labs/transcripts/exit-zero.bytes");
    const EXIT_NONZERO: &[u8] =
        include_bytes!("../../tests/fixtures/labs/transcripts/exit-nonzero.bytes");
    const NO_OSC_133: &[u8] =
        include_bytes!("../../tests/fixtures/labs/transcripts/no-osc-133.bytes");

    fn event_kinds(events: &[PromptEvent]) -> Vec<&'static str> {
        events
            .iter()
            .map(|e| match e {
                PromptEvent::PromptStart => "PromptStart",
                PromptEvent::CommandStart => "CommandStart",
                PromptEvent::OutputStart => "OutputStart",
                PromptEvent::CommandEnd { .. } => "CommandEnd",
            })
            .collect()
    }

    fn first_command_end(events: &[PromptEvent]) -> Option<i32> {
        events.iter().find_map(|e| match e {
            PromptEvent::CommandEnd { exit_code } => Some(*exit_code),
            _ => None,
        })?
    }

    /// LAB-06 — canonical OSC 133 sequence emits the four events in order
    /// and CommandEnd carries exit_code = Some(0).
    #[test]
    fn osc133_canonical() {
        let mut detector = PromptDetector::new();
        let events = detector.feed(KUBECTL_GET_PODS).unwrap();
        let kinds = event_kinds(&events);
        assert!(
            kinds.starts_with(&["PromptStart", "CommandStart", "OutputStart", "CommandEnd"]),
            "expected canonical event order, got {:?}",
            kinds
        );
        assert_eq!(
            first_command_end(&events),
            Some(0),
            "CommandEnd must carry exit_code=0"
        );
    }

    /// LAB-06 — exit-zero.bytes minimal sequence: A, B, C, D;0.
    #[test]
    fn osc133_canonical_exit_zero() {
        let mut detector = PromptDetector::new();
        let events = detector.feed(EXIT_ZERO).unwrap();
        assert_eq!(
            event_kinds(&events),
            vec!["PromptStart", "CommandStart", "OutputStart", "CommandEnd"]
        );
        assert_eq!(first_command_end(&events), Some(0));
    }

    /// LAB-06 — exit-nonzero.bytes: D;127 surfaces as Some(127).
    #[test]
    fn osc133_exit_nonzero() {
        let mut detector = PromptDetector::new();
        let events = detector.feed(EXIT_NONZERO).unwrap();
        assert_eq!(first_command_end(&events), Some(127));
    }

    /// LAB-06 — multi-line PS1 still produces one PromptStart per logical
    /// prompt (no spurious doubles when PS1 contains \n).
    #[test]
    fn osc133_multiline_ps1() {
        let mut detector = PromptDetector::new();
        let bytes = b"\x1b]133;A\x07user@host\n$ \x1b]133;B\x07echo hi\n\x1b]133;C\x07hi\n\x1b]133;D;0\x07";
        let events = detector.feed(bytes).unwrap();
        let starts = events
            .iter()
            .filter(|e| matches!(e, PromptEvent::PromptStart))
            .count();
        assert_eq!(starts, 1, "multi-line PS1 must emit exactly one PromptStart");
    }

    /// LAB-06 — full-screen apps (top, vim) cursor-redraw escape sequences
    /// must NOT trigger spurious CommandEnd events.
    #[test]
    fn top_vim_full_screen_apps_do_not_confuse_parser() {
        let mut detector = PromptDetector::new();
        // CSI sequences (ESC [ ...) and a non-133 OSC (ESC ] 52;A BEL) mixed in.
        let bytes = b"\x1b[H\x1b[2J Tasks: 100 total\nCPU: 12%\n\x1b]52;A\x07\x1b[H\x1b[2J Tasks: 100 total\nCPU: 13%\n";
        let events = detector.feed(bytes).unwrap();
        let ends = events
            .iter()
            .filter(|e| matches!(e, PromptEvent::CommandEnd { .. }))
            .count();
        assert_eq!(
            ends, 0,
            "non-133 escape sequences must not emit CommandEnd, got {}",
            ends
        );
    }

    /// LAB-06 — heuristic fallback emits CommandEnd { None } after
    /// HEURISTIC_TIMEOUT_SECS of silence on a stream without OSC 133.
    #[test]
    fn heuristic_fallback_after_timeout() {
        let mut detector = PromptDetector::new();
        let initial = detector.feed(NO_OSC_133).unwrap();
        assert_eq!(
            initial
                .iter()
                .filter(|e| matches!(e, PromptEvent::CommandEnd { .. }))
                .count(),
            0,
            "no-OSC-133 stream must NOT emit CommandEnd from feed alone"
        );
        let timed_out = detector.tick(HEURISTIC_TIMEOUT_SECS).unwrap();
        let end = timed_out
            .iter()
            .find(|e| matches!(e, PromptEvent::CommandEnd { .. }))
            .expect("heuristic must emit CommandEnd after timeout");
        match end {
            PromptEvent::CommandEnd { exit_code } => assert_eq!(*exit_code, None),
            _ => unreachable!(),
        }
    }

    /// LAB-06 — heuristic fallback does NOT fire before the timeout window.
    #[test]
    fn heuristic_fallback_does_not_fire_before_30s() {
        let mut detector = PromptDetector::new();
        let _ = detector.feed(NO_OSC_133).unwrap();
        let early = detector.tick(5).unwrap();
        assert!(
            early.is_empty(),
            "heuristic must NOT fire after only 5s of silence, got {:?}",
            early
        );
        // Accumulated silence still under threshold.
        let mid = detector.tick(20).unwrap();
        assert!(mid.is_empty(), "25s total must still be under threshold");
    }

    /// LAB-06 — heuristic fallback is suppressed once OSC 133 has been seen.
    /// Even if a long quiet period follows, we trust the explicit markers.
    #[test]
    fn heuristic_suppressed_after_osc133_seen() {
        let mut detector = PromptDetector::new();
        let _ = detector.feed(EXIT_ZERO).unwrap();
        // Long silence shouldn't fabricate a second CommandEnd.
        let later = detector.tick(120).unwrap();
        assert!(
            later
                .iter()
                .all(|e| !matches!(e, PromptEvent::CommandEnd { .. })),
            "heuristic must stay quiet once OSC 133 markers exist"
        );
    }

    /// LAB-06 — partial OSC sequences across feed boundaries still parse.
    #[test]
    fn osc133_split_across_feed_calls() {
        let mut detector = PromptDetector::new();
        // Split the canonical sequence in the middle of "133;A".
        let mut all = Vec::new();
        all.extend(detector.feed(b"\x1b]13").unwrap());
        all.extend(detector.feed(b"3;A\x07").unwrap());
        all.extend(detector.feed(b"$ \x1b]133;B\x07ls\n\x1b]133;C\x07").unwrap());
        all.extend(detector.feed(b"\x1b]133;D;0\x07").unwrap());
        assert_eq!(
            event_kinds(&all),
            vec!["PromptStart", "CommandStart", "OutputStart", "CommandEnd"]
        );
        assert_eq!(first_command_end(&all), Some(0));
    }
}
