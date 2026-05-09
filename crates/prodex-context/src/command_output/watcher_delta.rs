use super::*;

pub(super) fn compact_watcher_delta_output(
    input: &str,
    detected_kind: CommandOutputKind,
    options: &CommandOutputCompactOptions,
) -> Option<String> {
    let lines = command_lines(input);
    if !watcher_delta_kind_allowed(detected_kind) || lines.len() < 16 {
        return None;
    }

    let marker_count = lines
        .iter()
        .filter(|line| watcher_delta_marker_label(line).is_some())
        .count();
    let duplicate_keys = count_duplicate_watcher_delta_keys(&lines);
    let state_lines = lines
        .iter()
        .filter(|line| watcher_delta_state_label(line).is_some())
        .count();
    let failure_indices = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| is_watcher_delta_failure_line(line))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    if !watcher_delta_should_compact(
        detected_kind,
        lines.len(),
        options.max_lines,
        marker_count,
        duplicate_keys,
        state_lines,
        failure_indices.len(),
    ) {
        return None;
    }

    let mut state_counts = BTreeMap::<String, usize>::new();
    let mut last_state_by_entity = BTreeMap::<String, (usize, String)>::new();
    let mut state_changes = Vec::<(usize, String)>::new();
    let mut previous_status_by_entity = BTreeMap::<String, String>::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(status) = watcher_delta_state_label(line) else {
            continue;
        };
        *state_counts.entry(status.to_string()).or_default() += 1;
        let entity = watcher_delta_entity_key(line);
        let status = status.to_string();
        if previous_status_by_entity.get(&entity) != Some(&status) {
            state_changes.push((
                index,
                truncate_command_line(line.trim(), options.max_line_chars),
            ));
            previous_status_by_entity.insert(entity.clone(), status);
        }
        last_state_by_entity.insert(
            entity,
            (
                index,
                truncate_command_line(line.trim(), options.max_line_chars),
            ),
        );
    }

    let state_limit = options.max_lines.max(24).saturating_div(3).clamp(6, 24);
    let mut last_state_lines = select_watcher_delta_state_lines(
        state_changes,
        last_state_by_entity.into_values().collect(),
        state_limit,
    );
    if last_state_lines.is_empty() {
        last_state_lines = lines
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
            .take(state_limit)
            .map(|line| truncate_command_line(line.trim(), options.max_line_chars))
            .collect::<Vec<_>>();
        last_state_lines.reverse();
    }

    let failure_limit = options.max_lines.max(24).saturating_div(2).clamp(6, 32);
    let failure_lines = failure_indices
        .iter()
        .rev()
        .take(failure_limit)
        .map(|index| truncate_command_line(lines[*index].trim(), options.max_line_chars))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    let mut output = Vec::<String>::new();
    output.push(format!("pcs: watcher-delta ({}->sum)", lines.len()));
    output.push(format!(
        "sum: watcher/log lines={}, duplicate_keys={}, markers={}, state_lines={}, failures={}",
        lines.len(),
        duplicate_keys,
        marker_count,
        state_lines,
        failure_indices.len()
    ));
    if !state_counts.is_empty() {
        output.push(format_count_map("states", &state_counts, 10));
    }
    push_labeled_lines(
        &mut output,
        "last state changes",
        &last_state_lines,
        state_limit,
    );
    push_labeled_lines(
        &mut output,
        "failure/error lines",
        &failure_lines,
        failure_limit,
    );

    let text = lines_to_text(output);
    (text.len() < input.len()).then_some(text)
}

fn watcher_delta_kind_allowed(kind: CommandOutputKind) -> bool {
    !matches!(
        kind,
        CommandOutputKind::GitStatus
            | CommandOutputKind::GitDiff
            | CommandOutputKind::GitLog
            | CommandOutputKind::Search
            | CommandOutputKind::FileList
    )
}

fn watcher_delta_should_compact(
    kind: CommandOutputKind,
    line_count: usize,
    max_lines: usize,
    marker_count: usize,
    duplicate_keys: usize,
    state_lines: usize,
    failure_lines: usize,
) -> bool {
    let over_budget = line_count > max_lines.max(24);
    if marker_count > 0 {
        return over_budget || state_lines >= 6 || duplicate_keys >= 6 || failure_lines > 0;
    }
    if kind == CommandOutputKind::NoisySuccess {
        return false;
    }
    if kind == CommandOutputKind::LogStream {
        return over_budget && state_lines >= 16 && duplicate_keys >= 8;
    }
    over_budget && state_lines >= 16 && duplicate_keys >= 8
}

fn count_duplicate_watcher_delta_keys(lines: &[&str]) -> usize {
    let mut counts = BTreeMap::<String, usize>::new();
    for line in lines {
        let Some(key) = watcher_delta_line_key(line) else {
            continue;
        };
        *counts.entry(key).or_default() += 1;
    }
    counts.values().map(|count| count.saturating_sub(1)).sum()
}

fn watcher_delta_line_key(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.len() < 6 || is_generated_compaction_header_line(trimmed) {
        return None;
    }
    if trimmed
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .count()
        < 3
    {
        return None;
    }

    let mut key = String::with_capacity(trimmed.len());
    let mut previous_space = false;
    let mut previous_hash = false;
    for ch in trimmed.to_ascii_lowercase().chars() {
        if ch.is_ascii_digit() {
            if !previous_hash {
                key.push('#');
            }
            previous_hash = true;
            previous_space = false;
            continue;
        }
        previous_hash = false;
        if ch.is_ascii_alphabetic() || matches!(ch, '_' | '-' | '/' | '.' | ':') {
            key.push(ch);
            previous_space = false;
        } else if !previous_space {
            key.push(' ');
            previous_space = true;
        }
    }

    let key = key.split_whitespace().collect::<Vec<_>>().join(" ");
    (key.len() >= 6).then_some(key)
}

fn watcher_delta_marker_label(line: &str) -> Option<&'static str> {
    let lower = line.trim_start().to_ascii_lowercase();
    if lower.contains("refreshing run status")
        || lower.contains("gh run watch")
        || lower.contains("view this run in github")
    {
        Some("gh_run_watch")
    } else if lower.contains("press ctrl+c to quit")
        || lower.contains("watch usage")
        || lower.contains("watch mode")
        || lower.contains("watching for file changes")
        || lower.contains("waiting for file changes")
        || lower.contains("rerun when files change")
    {
        Some("watch")
    } else if lower.starts_with("==> ") && lower.ends_with(" <==")
        || lower.contains("tailing ")
        || lower.contains("following logs")
    {
        Some("log_tail")
    } else {
        None
    }
}

fn watcher_delta_state_label(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    if lower.is_empty() || is_generated_compaction_header_line(trimmed) {
        return None;
    }
    if is_watcher_delta_failure_line(line) {
        Some("failure")
    } else if lower.contains("cancelled") || lower.contains("canceled") {
        Some("cancelled")
    } else if lower.contains("skipped") || lower.contains("neutral") {
        Some("skipped")
    } else if trimmed.starts_with('✓')
        || lower.contains(" succeeded")
        || lower.contains(" success")
        || lower.contains(" successful")
        || lower.contains(" passed")
        || lower.ends_with(" ok")
    {
        Some("success")
    } else if lower.contains("queued")
        || lower.contains("pending")
        || lower.contains("waiting")
        || trimmed.starts_with("- ")
    {
        Some("queued")
    } else if lower.contains("in_progress")
        || lower.contains("in progress")
        || lower.contains(" running")
        || lower.starts_with("running ")
        || trimmed.starts_with("* ")
    {
        Some("running")
    } else {
        None
    }
}

fn is_watcher_delta_failure_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    is_error_signal_line(line)
        || is_test_failure_signal_line(line)
        || is_success_output_failure_signal_line(line)
        || lower.contains(" conclusion: failure")
        || lower.contains(" status: failure")
        || lower.contains("failure after ")
        || lower.contains("failed with exit code")
        || lower.contains("completed with exit code")
        || lower.contains("exited with code")
        || trimmed.starts_with("X ")
        || trimmed.starts_with('x') && lower.contains(" failed")
}

fn watcher_delta_entity_key(line: &str) -> String {
    let line = line
        .trim_start()
        .trim_start_matches(['*', '-', '!', 'X', 'x', '✓'])
        .trim_start();
    let mut key = watcher_delta_line_key(line).unwrap_or_else(|| line.trim().to_ascii_lowercase());
    for word in [
        "failure",
        "failed",
        "success",
        "successful",
        "succeeded",
        "passed",
        "queued",
        "pending",
        "waiting",
        "running",
        "in_progress",
        "in progress",
        "cancelled",
        "canceled",
        "skipped",
    ] {
        key = key.replace(word, " ");
    }
    key.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn select_watcher_delta_state_lines(
    mut state_changes: Vec<(usize, String)>,
    mut latest_states: Vec<(usize, String)>,
    limit: usize,
) -> Vec<String> {
    let limit = limit.max(1);
    state_changes.sort_by_key(|(index, _)| *index);
    latest_states.sort_by_key(|(index, _)| *index);

    let mut selected = BTreeMap::<usize, String>::new();
    for (index, line) in state_changes.into_iter().rev().take(limit.div_ceil(2)) {
        selected.insert(index, line);
    }
    for (index, line) in latest_states.into_iter().rev() {
        selected.insert(index, line);
        if selected.len() >= limit {
            break;
        }
    }

    let mut seen = BTreeMap::<String, ()>::new();
    selected
        .into_values()
        .filter(|line| {
            let key = critical_line_selection_key(line);
            seen.insert(key, ()).is_none()
        })
        .collect()
}
