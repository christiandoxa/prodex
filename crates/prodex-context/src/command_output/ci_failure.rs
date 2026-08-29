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
    let body_budget = options.max_lines.saturating_sub(4).clamp(6, 48);
    let selected = select_ci_failure_indices(&lines, &failure_indices, &job, &step, body_budget);

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
    push_ci_failure_slice(&mut output, &lines, &selected, options.max_line_chars);

    let text = lines_to_text(output);
    if text.len() < input.len() {
        Some(text)
    } else {
        None
    }
}

#[cfg(feature = "mojo")]
const CI_LINE_MARKER: i64 = 1;
#[cfg(feature = "mojo")]
const CI_LINE_ANNOTATION: i64 = 2;
#[cfg(feature = "mojo")]
const CI_LINE_JOB: i64 = 4;
#[cfg(feature = "mojo")]
const CI_LINE_STEP: i64 = 8;
#[cfg(feature = "mojo")]
const CI_LINE_EXIT_CODE: i64 = 16;
#[cfg(feature = "mojo")]
const CI_LINE_FAILURE_TEXT: i64 = 32;

#[cfg(feature = "mojo")]
fn mojo_ci_line(line: &str) -> [i64; 7] {
    prodex_mojo_core::context::classify_ci_line(line)
        .unwrap_or_else(|error| panic!("Mojo CI line classification failed: {error:?}"))
}

#[cfg(feature = "mojo")]
fn mojo_ci_line_span<'a>(line: &'a str, result: &[i64; 7], start: usize) -> Option<&'a str> {
    let end = start + 1;
    let start = usize::try_from(result[start]).ok()?;
    let end = usize::try_from(result[end]).ok()?;
    let value = line.get(start..end)?.trim();
    (!value.is_empty()).then_some(value)
}

#[cfg(feature = "mojo")]
fn is_ci_log_marker_line(line: &str) -> bool {
    mojo_ci_line(line)[0] & CI_LINE_MARKER != 0
}

#[cfg(not(feature = "mojo"))]
fn is_ci_log_marker_line(line: &str) -> bool {
    is_ci_log_marker_line_rust(line)
}

#[cfg(any(not(feature = "mojo"), test))]
fn is_ci_log_marker_line_rust(line: &str) -> bool {
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

#[cfg(feature = "mojo")]
fn is_ci_annotation_line(line: &str) -> bool {
    mojo_ci_line(line)[0] & CI_LINE_ANNOTATION != 0
}

#[cfg(not(feature = "mojo"))]
fn is_ci_annotation_line(line: &str) -> bool {
    is_ci_annotation_line_rust(line)
}

#[cfg(any(not(feature = "mojo"), test))]
fn is_ci_annotation_line_rust(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    lower.starts_with("##[error]") || lower.starts_with("::error")
}

fn select_ci_failure_indices(
    lines: &[&str],
    failure_indices: &[usize],
    job: &Option<(usize, String)>,
    step: &Option<(usize, String)>,
    body_budget: usize,
) -> Vec<usize> {
    let mut selected = BTreeSet::<usize>::new();
    if let Some((index, _)) = job {
        selected.insert(*index);
    }
    if let Some((index, _)) = step {
        selected.insert(*index);
    }
    for index in failure_indices {
        let start = index.saturating_sub(2);
        let end = index.saturating_add(2).min(lines.len().saturating_sub(1));
        for selected_index in start..=end {
            selected.insert(selected_index);
        }
    }
    trim_ci_selected_indices(lines, selected, failure_indices, body_budget)
}

fn push_ci_failure_slice(
    output: &mut Vec<String>,
    lines: &[&str],
    selected: &[usize],
    max_line_chars: usize,
) {
    let mut previous = None::<usize>;
    for index in selected {
        if let Some(previous_index) = previous {
            let omitted = index.saturating_sub(previous_index).saturating_sub(1);
            if omitted > 0 {
                output.push(format!("[... omitted {omitted} ci log lines ...]"));
            }
        }
        output.push(truncate_command_line(lines[*index], max_line_chars));
        previous = Some(*index);
    }
}

#[cfg(feature = "mojo")]
fn is_ci_failure_signal_line(line: &str) -> bool {
    let flags = mojo_ci_line(line)[0];
    flags & (CI_LINE_ANNOTATION | CI_LINE_FAILURE_TEXT) != 0
        || is_error_signal_line(line)
        || is_test_failure_signal_line(line)
        || is_success_output_failure_signal_line(line)
}

#[cfg(not(feature = "mojo"))]
fn is_ci_failure_signal_line(line: &str) -> bool {
    is_ci_failure_signal_line_rust(line)
}

#[cfg(any(not(feature = "mojo"), test))]
fn is_ci_failure_signal_line_rust(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    is_ci_annotation_line_rust(line)
        || lower.contains("process completed with exit code")
        || lower.contains("failed with exit code")
        || lower.contains("exited with code")
        || lower.contains("exit status")
        || is_error_signal_line(line)
        || is_test_failure_signal_line(line)
        || is_success_output_failure_signal_line(line)
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

#[cfg(feature = "mojo")]
fn ci_job_name_from_line(line: &str) -> Option<String> {
    let result = mojo_ci_line(line);
    if result[0] & CI_LINE_JOB == 0 {
        return None;
    }
    mojo_ci_line_span(line, &result, 1).map(|value| truncate_command_line(value, 96))
}

#[cfg(not(feature = "mojo"))]
fn ci_job_name_from_line(line: &str) -> Option<String> {
    ci_job_name_from_line_rust(line)
}

#[cfg(any(not(feature = "mojo"), test))]
fn ci_job_name_from_line_rust(line: &str) -> Option<String> {
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

#[cfg(feature = "mojo")]
fn ci_step_name_from_line(line: &str) -> Option<String> {
    let result = mojo_ci_line(line);
    if result[0] & CI_LINE_STEP == 0 {
        return None;
    }
    mojo_ci_line_span(line, &result, 3).map(|value| truncate_command_line(value, 120))
}

#[cfg(not(feature = "mojo"))]
fn ci_step_name_from_line(line: &str) -> Option<String> {
    ci_step_name_from_line_rust(line)
}

#[cfg(any(not(feature = "mojo"), test))]
fn ci_step_name_from_line_rust(line: &str) -> Option<String> {
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

#[cfg(feature = "mojo")]
fn ci_exit_code_from_line(line: &str) -> Option<String> {
    let result = mojo_ci_line(line);
    if result[0] & CI_LINE_EXIT_CODE == 0 {
        return None;
    }
    mojo_ci_line_span(line, &result, 5).map(str::to_string)
}

#[cfg(not(feature = "mojo"))]
fn ci_exit_code_from_line(line: &str) -> Option<String> {
    ci_exit_code_from_line_rust(line)
}

#[cfg(any(not(feature = "mojo"), test))]
fn ci_exit_code_from_line_rust(line: &str) -> Option<String> {
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

#[cfg(any(not(feature = "mojo"), test))]
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

#[cfg(all(test, feature = "mojo"))]
mod mojo_ci_line_tests {
    use super::*;

    #[test]
    fn mojo_ci_line_metadata_matches_rust_oracle() {
        let cases = [
            "##[group]Run cargo test --workspace",
            "##[endgroup]",
            "##[error]Process completed with exit code 101.",
            "::error file=src/lib.rs::broken",
            " Current runner version: '2.0'",
            "runner name: linux",
            "runner os: Linux",
            "prepare workflow directory",
            "prepare all required actions",
            "complete job",
            "set up job",
            "actions/checkout@v4",
            "/_actions/prodex/test",
            "GitHub Actions",
            "job: test-linux",
            "workflow job: unit",
            "job test failed",
            "##[group]step: cargo test",
            "##[group]failed step: cargo test",
            "##[GROUP]Run uppercase group is not a step",
            "Process failed with code -1",
            "exit code unavailable; exit status: 2",
            "failed with code unavailable; code 4",
            "exit status: 101",
            "EXITED WITH CODE 7",
            "\u{2003}job:\u{2003}unicode-linux",
            "ordinary output",
        ];

        for line in cases {
            assert_eq!(
                is_ci_log_marker_line(line),
                is_ci_log_marker_line_rust(line),
                "marker: {line:?}"
            );
            assert_eq!(
                is_ci_annotation_line(line),
                is_ci_annotation_line_rust(line),
                "annotation: {line:?}"
            );
            assert_eq!(
                is_ci_failure_signal_line(line),
                is_ci_failure_signal_line_rust(line),
                "failure: {line:?}"
            );
            assert_eq!(
                ci_job_name_from_line(line),
                ci_job_name_from_line_rust(line),
                "job: {line:?}"
            );
            assert_eq!(
                ci_step_name_from_line(line),
                ci_step_name_from_line_rust(line),
                "step: {line:?}"
            );
            assert_eq!(
                ci_exit_code_from_line(line),
                ci_exit_code_from_line_rust(line),
                "exit: {line:?}"
            );
        }
    }
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
