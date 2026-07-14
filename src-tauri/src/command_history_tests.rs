//! Tests for the Phase 19.3 (D-01) per-session CommandRecord history in
//! `lib.rs`. Extracted to a sibling file so `lib.rs` stays under the
//! 500-line CLAUDE.md cap — included via
//! `#[cfg(test)] #[path = "command_history_tests.rs"] mod command_history_tests;`
//! (same convention as `labs::spec` / `commands::labs::eval`).

use super::*;

fn record(command: &str, output: &str, exit_code: Option<i32>) -> CommandRecord {
    CommandRecord {
        command: command.to_string(),
        output: output.to_string(),
        exit_code,
    }
}

/// D-01 — pushing past the 200-record cap evicts oldest-first so len
/// stays <= LAB_HISTORY_MAX_RECORDS; the 201st push must drop record #1.
#[test]
fn push_command_record_evicts_oldest_over_record_cap() {
    let mut history: Vec<CommandRecord> = Vec::new();
    for i in 0..LAB_HISTORY_MAX_RECORDS {
        push_command_record(&mut history, record(&format!("cmd-{}", i), "out", Some(0)));
    }
    assert_eq!(history.len(), LAB_HISTORY_MAX_RECORDS);
    // One more push: len must stay capped and the oldest (cmd-0) evicted.
    push_command_record(
        &mut history,
        record("cmd-overflow", "out", Some(0)),
    );
    assert_eq!(history.len(), LAB_HISTORY_MAX_RECORDS);
    assert!(
        !history.iter().any(|r| r.command == "cmd-0"),
        "oldest record (cmd-0) must be evicted first"
    );
    assert!(
        history.iter().any(|r| r.command == "cmd-overflow"),
        "newest record must be retained"
    );
}

/// 19.3-REVIEW CR-03 — the byte budget must count COMMAND bytes too, not
/// only output bytes: records whose cumulative command+output bytes exceed
/// 1 MiB must evict oldest-first even when output alone stays under the cap.
#[test]
fn push_command_record_counts_command_bytes_toward_cap() {
    let mut history: Vec<CommandRecord> = Vec::new();
    // Shape chosen so OUTPUT bytes alone stay under the 1 MiB session cap
    // (and each output under the 256 KiB per-record cap — no truncation)
    // while command+output bytes exceed the session cap — the eviction
    // only fires if command bytes are counted:
    //   4 records of (~4 KiB cmd + 255 KiB out).
    //   Output total  = 4x255 KiB          = 1020 KiB <= 1024 KiB cap.
    //   Cmd+out total = 1020 KiB + 4x~4KiB ~ 1036 KiB  > 1024 KiB cap.
    let big_command = "c".repeat(4 * 1024 - 8); // just under the 4 KiB cap
    let output = "o".repeat(255 * 1024); // under the 256 KiB per-record cap
    for i in 0..4 {
        push_command_record(
            &mut history,
            record(&format!("{}-{}", big_command, i), &output, Some(0)),
        );
    }
    let total_bytes: usize = history
        .iter()
        .map(|r| r.command.len() + r.output.len())
        .sum();
    assert!(
        total_bytes <= LAB_HISTORY_MAX_BYTES,
        "cumulative command+output bytes ({}) must stay <= {} — command bytes \
         must count toward the D-01 budget",
        total_bytes,
        LAB_HISTORY_MAX_BYTES
    );
    assert!(
        history.len() < 4,
        "the oldest record must have been evicted to satisfy the combined cap"
    );
}

/// 19.3-REVIEW CR-03 — a single absurdly long command (multi-megabyte paste
/// or hostile renderer) must be truncated to the per-command cap before
/// insertion so 200 records x unbounded command bytes cannot grow resident
/// memory unboundedly.
#[test]
fn push_command_record_truncates_oversized_command() {
    let mut history: Vec<CommandRecord> = Vec::new();
    let huge_command = "x".repeat(3 * 1024 * 1024); // 3 MiB paste
    push_command_record(&mut history, record(&huge_command, "out", Some(0)));
    assert_eq!(history.len(), 1);
    assert!(
        history[0].command.len() <= LAB_HISTORY_MAX_COMMAND_BYTES,
        "command must be truncated to the per-command cap ({}), got {}",
        LAB_HISTORY_MAX_COMMAND_BYTES,
        history[0].command.len()
    );
}

/// 19.3-REVIEW CR-03 — a single record's output is truncated to the
/// per-record output cap (256 KiB) before insertion.
#[test]
fn push_command_record_truncates_oversized_output() {
    let mut history: Vec<CommandRecord> = Vec::new();
    let huge_output = "y".repeat(2 * 1024 * 1024); // 2 MiB output
    push_command_record(&mut history, record("cmd", &huge_output, Some(0)));
    assert_eq!(history.len(), 1);
    assert!(
        history[0].output.len() <= LAB_HISTORY_MAX_OUTPUT_BYTES,
        "output must be truncated to the per-record output cap ({}), got {}",
        LAB_HISTORY_MAX_OUTPUT_BYTES,
        history[0].output.len()
    );
}

/// D-01 — pushing records whose cumulative output bytes exceed 1 MiB
/// evicts oldest-first until the cumulative bound holds.
#[test]
fn push_command_record_evicts_over_byte_cap() {
    let mut history: Vec<CommandRecord> = Vec::new();
    // Each record's output is 300 KiB; 4 records exceed the 1 MiB cap.
    let big_output = "x".repeat(300 * 1024);
    for i in 0..4 {
        push_command_record(
            &mut history,
            record(&format!("cmd-{}", i), &big_output, Some(0)),
        );
    }
    let total_bytes: usize = history.iter().map(|r| r.output.len()).sum();
    assert!(
        total_bytes <= LAB_HISTORY_MAX_BYTES,
        "cumulative output bytes ({}) must stay <= {} after eviction",
        total_bytes,
        LAB_HISTORY_MAX_BYTES
    );
    assert!(
        !history.iter().any(|r| r.command == "cmd-0"),
        "oldest record must be evicted first to satisfy the byte cap"
    );
}
