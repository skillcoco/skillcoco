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
