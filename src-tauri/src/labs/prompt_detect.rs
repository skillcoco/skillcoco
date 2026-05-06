//! # labs::prompt_detect — OSC 133 + heuristic prompt-boundary detection
//! (Phase 03.1, Wave 0 stub)
//!
//! Wave 1 (03.1-02) implements:
//! - OSC 133 A (PromptStart) / B (CommandStart) / C (OutputStart) / D ;<exit>
//!   (CommandEnd) parser, fed byte-by-byte from the PTY read loop
//! - Heuristic fallback: regex on `^\$\s*$` (or `^# `, `^> `) lines after
//!   `HEURISTIC_TIMEOUT_SECS` of silence; emits CommandEnd { exit_code: None }
//! - Top/vim full-screen-app guard so terminal redraws don't trip the
//!   heuristic
//!
//! Wave 0: byte-stream fixtures are committed under
//! `src-tauri/tests/fixtures/labs/transcripts/` so 03.1-02 has a
//! deterministic test bed.

use super::LabError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptEvent {
    PromptStart,
    CommandStart,
    OutputStart,
    CommandEnd { exit_code: Option<i32> },
}

/// Streaming detector. Wave 1 fills in the impl; Wave 0 keeps the shape.
pub struct PromptDetector {
    _wave_0_placeholder: (),
}

impl PromptDetector {
    pub fn new() -> Self {
        Self {
            _wave_0_placeholder: (),
        }
    }

    /// Feed bytes into the detector and drain any emitted events.
    /// Wave 0 stub returns Err.
    pub fn feed(&mut self, _bytes: &[u8]) -> Result<Vec<PromptEvent>, LabError> {
        Err(LabError::Eval(
            "PromptDetector::feed: implemented in 03.1-02".to_string(),
        ))
    }

    /// Indicate that `seconds` of silence have elapsed (heuristic
    /// fallback). Wave 0 stub returns Err.
    pub fn tick(&mut self, _seconds: u64) -> Result<Vec<PromptEvent>, LabError> {
        Err(LabError::Eval(
            "PromptDetector::tick: implemented in 03.1-02".to_string(),
        ))
    }
}

impl Default for PromptDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KUBECTL_GET_PODS: &[u8] =
        include_bytes!("../../tests/fixtures/labs/transcripts/kubectl-get-pods.bytes");
    const NO_OSC_133: &[u8] =
        include_bytes!("../../tests/fixtures/labs/transcripts/no-osc-133.bytes");

    /// LAB-06 — canonical OSC 133 sequence emits the four events in order.
    #[test]
    fn osc133_canonical() {
        let mut detector = PromptDetector::new();
        let events = detector
            .feed(KUBECTL_GET_PODS)
            .expect("PromptDetector::feed must succeed once 03.1-02 lands");

        // Filter to the first four distinct events (the fixture ends with
        // a follow-up PromptStart + CommandStart for the next prompt).
        let kinds: Vec<&str> = events
            .iter()
            .map(|e| match e {
                PromptEvent::PromptStart => "PromptStart",
                PromptEvent::CommandStart => "CommandStart",
                PromptEvent::OutputStart => "OutputStart",
                PromptEvent::CommandEnd { .. } => "CommandEnd",
            })
            .collect();
        assert!(
            kinds.starts_with(&["PromptStart", "CommandStart", "OutputStart", "CommandEnd"]),
            "expected canonical event order, got {:?}",
            kinds
        );

        let end = events
            .iter()
            .find(|e| matches!(e, PromptEvent::CommandEnd { .. }))
            .expect("must include CommandEnd");
        match end {
            PromptEvent::CommandEnd { exit_code } => {
                assert_eq!(*exit_code, Some(0), "CommandEnd must carry exit_code=0");
            }
            _ => unreachable!(),
        }
    }

    /// LAB-06 — multi-line PS1 still produces one PromptStart per logical
    /// prompt (no spurious doubles when PS1 contains \n).
    #[test]
    fn osc133_multiline_ps1() {
        // Wave 1 will use a fixture with \n in PS1 between OSC A and B.
        // For Wave 0 we feed an inline byte sequence and assert the impl
        // handles it.
        let mut detector = PromptDetector::new();
        let bytes = b"\x1b]133;A\x07user@host\n$ \x1b]133;B\x07echo hi\n\x1b]133;C\x07hi\n\x1b]133;D;0\x07";
        let events = detector
            .feed(bytes)
            .expect("PromptDetector::feed must succeed once 03.1-02 lands");
        let prompt_starts = events
            .iter()
            .filter(|e| matches!(e, PromptEvent::PromptStart))
            .count();
        assert_eq!(
            prompt_starts, 1,
            "multi-line PS1 must emit exactly one PromptStart, got {}",
            prompt_starts
        );
    }

    /// LAB-06 — heuristic fallback emits CommandEnd { exit_code: None }
    /// after a timeout of silence on a stream without OSC 133.
    #[test]
    fn heuristic_fallback_after_timeout() {
        let mut detector = PromptDetector::new();
        let initial = detector
            .feed(NO_OSC_133)
            .expect("feed must succeed once 03.1-02 lands");
        // No OSC 133 in the byte stream, so feed() should produce zero
        // CommandEnd events on its own.
        let initial_ends = initial
            .iter()
            .filter(|e| matches!(e, PromptEvent::CommandEnd { .. }))
            .count();
        assert_eq!(
            initial_ends, 0,
            "no-OSC-133 stream must NOT emit CommandEnd from feed alone"
        );

        // After 30s of silence the heuristic fires.
        let timed_out = detector
            .tick(30)
            .expect("tick must succeed once 03.1-02 lands");
        let end = timed_out
            .iter()
            .find(|e| matches!(e, PromptEvent::CommandEnd { .. }))
            .expect("heuristic must emit CommandEnd after 30s");
        match end {
            PromptEvent::CommandEnd { exit_code } => {
                assert_eq!(
                    *exit_code, None,
                    "heuristic fallback cannot know exit code"
                );
            }
            _ => unreachable!(),
        }
    }

    /// LAB-06 — full-screen apps (top, vim) must not confuse the heuristic
    /// with their cursor-redraw escape sequences.
    #[test]
    fn top_vim_full_screen_apps_do_not_confuse_parser() {
        let mut detector = PromptDetector::new();
        // Simulate a top-style redraw: cursor home + clear + repaint.
        let bytes = b"\x1b[H\x1b[2J Tasks: 100 total\nCPU: 12%\n\x1b[H\x1b[2J Tasks: 100 total\nCPU: 13%\n";
        let events = detector
            .feed(bytes)
            .expect("feed must succeed once 03.1-02 lands");
        let ends = events
            .iter()
            .filter(|e| matches!(e, PromptEvent::CommandEnd { .. }))
            .count();
        assert_eq!(
            ends, 0,
            "top/vim redraw bytes must NOT trigger spurious CommandEnd, got {}",
            ends
        );
    }
}
