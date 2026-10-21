use super::{RunState, helpers::prune_terminal_runs, tests::record};
use std::collections::BTreeMap;

#[test]
fn run_retention_keeps_32_terminal_records_and_all_live_states() {
    let mut runs = BTreeMap::new();
    for index in 0..36 {
        let mut row = record();
        row.created_at = index;
        row.finished_at = Some(index);
        row.state = [
            RunState::Succeeded,
            RunState::Failed,
            RunState::Cancelled,
            RunState::StartFailed,
        ][index as usize % 4];
        runs.insert(format!("terminal-{index:02}"), row);
    }
    for (key, state) in [
        ("queued", RunState::Queued),
        ("starting", RunState::Starting),
        ("running", RunState::Running),
    ] {
        let mut row = record();
        row.created_at = 0;
        row.state = state;
        runs.insert(key.to_string(), row);
    }
    prune_terminal_runs(&mut runs);
    let expected = [
        "queued".to_string(),
        "running".to_string(),
        "starting".to_string(),
    ]
    .into_iter()
    .chain((4..36).map(|index| format!("terminal-{index:02}")))
    .collect::<Vec<_>>();
    assert_eq!(runs.keys().cloned().collect::<Vec<_>>(), expected);
    assert_eq!(runs["queued"].state, RunState::Queued);
    assert_eq!(runs["running"].state, RunState::Running);
    assert_eq!(runs["starting"].state, RunState::Starting);
}

#[test]
fn run_retention_uses_finished_fallback_created_and_lexical_ties() {
    let mut runs = BTreeMap::new();
    for index in 0..30 {
        let mut row = record();
        row.state = RunState::Succeeded;
        row.created_at = u64::MAX - index;
        row.finished_at = Some(u64::MAX - index);
        runs.insert(format!("new-{index:02}"), row);
    }
    for (key, created_at, finished_at) in [
        ("a", 9, Some(4)),
        ("b", 1, Some(4)),
        ("c", 1, Some(4)),
        ("d", 2, None),
    ] {
        let mut row = record();
        row.state = RunState::Failed;
        row.created_at = created_at;
        row.finished_at = finished_at;
        runs.insert(key.into(), row);
    }
    prune_terminal_runs(&mut runs);
    assert!(runs.contains_key("a"));
    assert!(!runs.contains_key("b"));
    assert!(runs.contains_key("c"));
    assert!(!runs.contains_key("d"));
    assert_eq!(runs.len(), 32);
    let keys = runs.keys().cloned().collect::<Vec<_>>();
    prune_terminal_runs(&mut runs);
    assert_eq!(runs.keys().cloned().collect::<Vec<_>>(), keys);
}
