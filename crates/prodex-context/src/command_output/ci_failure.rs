use super::*;
use std::collections::BTreeSet;

pub(super) fn compact_ci_failure_log_output(
    input: &str,
    options: &CommandOutputCompactOptions,
) -> Option<String> {
    let lines = command_lines(input);
    if lines.len() < 12 {
        return None;
    }

    let ci_markers = lines
        .iter()
        .filter(|line| is_ci_log_marker_line(line))
        .count();
    let failure_indices = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| is_ci_failure_signal_line(line))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if failure_indices.is_empty()
        || ci_markers < 2
            && !failure_indices
                .iter()
                .any(|index| is_ci_annotation_line(lines[*index]))
    {
        return None;
    }

    let first_failure = *failure_indices.first().unwrap_or(&0);
    let job = ci_job_before(&lines, first_failure);
    let step = ci_step_before(&lines, first_failure);
    let exit_code = failure_indices
        .iter()
        .find_map(|index| ci_exit_code_from_line(lines[*index]));
    let mut selected = BTreeSet::<usize>::new();
    if let Some((index, _)) = &job {
        selected.insert(*index);
    }
    if let Some((index, _)) = &step {
        selected.insert(*index);
    }
    for index in &failure_indices {
        let start = index.saturating_sub(2);
        let end = index.saturating_add(2).min(lines.len().saturating_sub(1));
        for selected_index in start..=end {
            selected.insert(selected_index);
        }
    }

    let body_budget = options.max_lines.saturating_sub(4).clamp(6, 48);
    let selected = trim_ci_selected_indices(&lines, selected, &failure_indices, body_budget);

    let mut output = Vec::<String>::new();
    output.push(format!("pcs: ci-failure ({}->sum)", lines.len()));
    output.push(format!(
        "failed: job={} step={} exit={}",
        job.as_ref()
            .map(|(_, value)| value.as_str())
            .unwrap_or("(unknown)"),
        step.as_ref()
            .map(|(_, value)| value.as_str())
            .unwrap_or("(unknown)"),
        exit_code.as_deref().unwrap_or("(unknown)")
    ));
    output.push(format!(
        "sum: ci markers={}, failure_lines={}, selected_lines={}",
        ci_markers,
        failure_indices.len(),
        selected.len()
    ));
    output.push("failure slice:".to_string());
    let mut previous = None::<usize>;
    for index in selected {
        if let Some(previous_index) = previous {
            let omitted = index.saturating_sub(previous_index).saturating_sub(1);
            if omitted > 0 {
                output.push(format!("[... omitted {omitted} ci log lines ...]"));
            }
        }
        output.push(truncate_command_line(lines[index], options.max_line_chars));
        previous = Some(index);
    }

    let text = lines_to_text(output);
    if text.len() < input.len() {
        Some(text)
    } else {
        None
    }
}

fn is_ci_log_marker_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    lower.starts_with("##[group]")
        || lower.starts_with("##[endgroup]")
        || lower.starts_with("##[error]")
        || lower.starts_with("::error")
        || lower.starts_with("current runner version:")
        || lower.starts_with("runner name:")
        || lower.starts_with("runner os:")
        || lower.starts_with("prepare workflow directory")
        || lower.starts_with("prepare all required actions")
        || lower.starts_with("complete job")
        || lower.starts_with("set up job")
        || lower.contains("actions/checkout")
        || lower.contains("/_actions/")
        || lower.contains("github actions")
        || lower.contains("process completed with exit code")
}

fn is_ci_failure_signal_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    is_ci_annotation_line(line)
        || lower.contains("process completed with exit code")
        || lower.contains("failed with exit code")
        || lower.contains("exited with code")
        || lower.contains("exit status")
        || is_error_signal_line(line)
        || is_test_failure_signal_line(line)
        || is_success_output_failure_signal_line(line)
}

fn is_ci_annotation_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    lower.starts_with("##[error]") || lower.starts_with("::error")
}

fn ci_job_before(lines: &[&str], failure_index: usize) -> Option<(usize, String)> {
    lines
        .iter()
        .take(failure_index.saturating_add(1))
        .enumerate()
        .rev()
        .find_map(|(index, line)| ci_job_name_from_line(line).map(|name| (index, name)))
}

fn ci_step_before(lines: &[&str], failure_index: usize) -> Option<(usize, String)> {
    lines
        .iter()
        .take(failure_index.saturating_add(1))
        .enumerate()
        .rev()
        .find_map(|(index, line)| ci_step_name_from_line(line).map(|name| (index, name)))
}

fn ci_job_name_from_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    for prefix in ["job:", "job name:", "workflow job:", "failed job:"] {
        if lower.starts_with(prefix) {
            let name = trimmed[prefix.len()..].trim();
            return (!name.is_empty()).then(|| truncate_command_line(name, 96));
        }
    }
    if lower.starts_with("job ") && lower.contains(" failed") {
        return Some(truncate_command_line(trimmed, 96));
    }
    None
}

fn ci_step_name_from_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let body = trimmed.strip_prefix("##[group]").unwrap_or(trimmed).trim();
    let lower = body.to_ascii_lowercase();
    if lower.starts_with("run ") {
        let step = body[4..].trim();
        return (!step.is_empty()).then(|| truncate_command_line(step, 120));
    }
    for prefix in ["step:", "failed step:"] {
        if lower.starts_with(prefix) {
            let step = body[prefix.len()..].trim();
            return (!step.is_empty()).then(|| truncate_command_line(step, 120));
        }
    }
    None
}

fn ci_exit_code_from_line(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    for needle in [
        "exit code",
        "exit status",
        "exited with code",
        "failed with code",
        "code",
    ] {
        if let Some((_, after)) = lower.split_once(needle)
            && let Some(code) = first_integer_token(after)
        {
            return Some(code);
        }
    }
    None
}

fn first_integer_token(input: &str) -> Option<String> {
    let mut digits = String::new();
    let mut started = false;
    for ch in input.chars() {
        if ch.is_ascii_digit() || !started && ch == '-' {
            digits.push(ch);
            started = true;
        } else if started {
            break;
        }
    }
    let has_digit = digits.chars().any(|ch| ch.is_ascii_digit());
    has_digit.then_some(digits)
}

fn trim_ci_selected_indices(
    lines: &[&str],
    selected: BTreeSet<usize>,
    failure_indices: &[usize],
    budget: usize,
) -> Vec<usize> {
    if selected.len() <= budget {
        return selected.into_iter().collect();
    }

    let mut scored = selected
        .into_iter()
        .map(|index| {
            let nearest_failure = failure_indices
                .iter()
                .map(|failure| failure.abs_diff(index))
                .min()
                .unwrap_or(usize::MAX);
            let priority = if failure_indices.contains(&index) {
                0
            } else if ci_job_name_from_line(lines[index]).is_some()
                || ci_step_name_from_line(lines[index]).is_some()
            {
                1
            } else if is_critical_preserve_line(lines[index]) || is_ci_log_marker_line(lines[index])
            {
                2
            } else {
                3
            };
            (priority, nearest_failure, index)
        })
        .collect::<Vec<_>>();
    scored.sort_by_key(|(priority, distance, index)| (*priority, *distance, *index));
    let mut selected = scored
        .into_iter()
        .take(budget.max(1))
        .map(|(_, _, index)| index)
        .collect::<Vec<_>>();
    selected.sort_unstable();
    selected
}
