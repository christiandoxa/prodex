use crate::blob_noise::{
    context_noise_normalize_path_token_supplement,
    context_noise_strip_path_location_suffix_supplement,
};
use crate::critical_signal::{
    CriticalSignalCounts, count_critical_signals, count_file_location_signals,
    critical_signal_self_check, is_diff_hunk_line, is_error_signal_line,
    is_rust_diagnostic_signal_line, is_stack_signal_line, is_test_failure_signal_line,
    looks_like_location_path,
};
use crate::estimate_context_tokens;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::BTreeMap;

mod ci_failure;
mod critical_blocks;
mod git_search;
mod git_search_parse;
mod intent;
mod kind_detection;
mod log_stream;
mod noisy_success;
mod path_aliases;
mod structured_json;
mod success;

pub use intent::extract_intent_terms_from_prompt;
pub use kind_detection::infer_command_output_kind_from_metadata;
pub(crate) use log_stream::is_log_level_signal_line;
pub use success::compact_successful_command_output_with_options;

use ci_failure::compact_ci_failure_log_output;
pub(crate) use critical_blocks::*;
use git_search::{
    collect_file_list_entries, collect_search_output_matches, compact_file_list_output,
    compact_git_diff_output, compact_git_diff_output_with_intent, compact_git_log_stat_output,
    compact_git_status_output, compact_search_output,
};
pub(crate) use git_search_parse::*;
use intent::{
    compact_command_output_for_intent, ensure_no_critical_signal_loss_for_intent,
    intent_line_matches, normalize_intent_terms_with_prompt_expansion, score_intent_text,
};
use kind_detection::{
    command_metadata_subcommand_after, command_metadata_token_command_name,
    command_metadata_tokens, detect_command_output_kind_with_hint,
};
use log_stream::compact_log_stream_output;
pub(crate) use noisy_success::*;
use path_aliases::canonicalize_compacted_command_paths;
use structured_json::compact_structured_json_output;
use success::{count_success_output_path_extensions, count_success_output_path_roots};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandOutputKind {
    Auto,
    GitStatus,
    GitDiff,
    RustDiagnostics,
    Diagnostics,
    GitLog,
    Search,
    FileList,
    LogStream,
    NoisySuccess,
    Plain,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandOutputCompactOptions {
    pub kind: CommandOutputKind,
    pub max_lines: usize,
    pub head_lines: usize,
    pub tail_lines: usize,
    pub max_line_chars: usize,
    pub max_search_matches_per_file: usize,
    pub max_path_entries: usize,
}

impl Default for CommandOutputCompactOptions {
    fn default() -> Self {
        Self {
            kind: CommandOutputKind::Auto,
            max_lines: 160,
            head_lines: 80,
            tail_lines: 40,
            max_line_chars: 240,
            max_search_matches_per_file: 4,
            max_path_entries: 120,
        }
    }
}

impl CommandOutputCompactOptions {
    #[allow(clippy::too_many_arguments)]
    pub fn from_limits(
        kind: CommandOutputKind,
        max_lines: usize,
        head_lines: usize,
        tail_lines: usize,
        max_line_chars: usize,
        max_search_matches_per_file: usize,
        max_path_entries: usize,
    ) -> Self {
        Self {
            kind,
            max_lines,
            head_lines,
            tail_lines,
            max_line_chars,
            max_search_matches_per_file,
            max_path_entries,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CommandOutputIntentCompactOptions {
    pub base: CommandOutputCompactOptions,
    pub intent_terms: Vec<String>,
    pub kind_hint: Option<CommandOutputKind>,
}

impl CommandOutputIntentCompactOptions {
    pub fn new(base: CommandOutputCompactOptions, intent_terms: Vec<String>) -> Self {
        Self {
            base,
            intent_terms,
            kind_hint: None,
        }
    }

    pub fn with_kind_hint(mut self, kind_hint: Option<CommandOutputKind>) -> Self {
        self.kind_hint = kind_hint;
        self
    }
}

pub const MAX_EXTRACTED_INTENT_TERMS: usize = 32;

#[derive(Debug, Clone, Serialize)]
pub struct CommandOutputCompactReport {
    pub requested_kind: CommandOutputKind,
    pub detected_kind: CommandOutputKind,
    pub original_lines: usize,
    pub compacted_lines: usize,
    pub estimated_tokens_before: usize,
    pub estimated_tokens_after: usize,
    pub output: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandSuccessOutputCompactOptions {
    pub command: Option<String>,
    pub exit_code: Option<i32>,
    pub min_lines_to_compact: usize,
    pub max_touched_files: usize,
    pub max_key_lines: usize,
    pub max_line_chars: usize,
}

impl Default for CommandSuccessOutputCompactOptions {
    fn default() -> Self {
        Self {
            command: None,
            exit_code: None,
            min_lines_to_compact: 40,
            max_touched_files: 24,
            max_key_lines: 12,
            max_line_chars: 200,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandSuccessOutputCompactReport {
    pub compacted: bool,
    pub failure_suspected: bool,
    pub original_lines: usize,
    pub compacted_lines: usize,
    pub touched_files: usize,
    pub critical_signals: CriticalSignalCounts,
    pub output: String,
}

impl CommandOutputKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            CommandOutputKind::Auto => "auto",
            CommandOutputKind::GitStatus => "git-status",
            CommandOutputKind::GitDiff => "git-diff",
            CommandOutputKind::RustDiagnostics => "rust-diag",
            CommandOutputKind::Diagnostics => "diag",
            CommandOutputKind::GitLog => "git-log",
            CommandOutputKind::Search => "search",
            CommandOutputKind::FileList => "files",
            CommandOutputKind::LogStream => "logs",
            CommandOutputKind::NoisySuccess => "success",
            CommandOutputKind::Plain => "plain",
        }
    }
}

pub fn compact_command_output(input: &str, kind: CommandOutputKind) -> String {
    let options = CommandOutputCompactOptions {
        kind,
        ..CommandOutputCompactOptions::default()
    };
    compact_command_output_with_options(input, &options).output
}

pub fn command_output_kind_hint_for_command(command: &str) -> Option<CommandOutputKind> {
    infer_command_output_kind_from_metadata(command)
}

pub fn compact_command_output_with_options(
    input: &str,
    options: &CommandOutputCompactOptions,
) -> CommandOutputCompactReport {
    compact_command_output_with_options_and_kind_hint(input, options, None)
}

pub fn compact_command_output_with_options_and_kind_hint(
    input: &str,
    options: &CommandOutputCompactOptions,
    kind_hint: Option<CommandOutputKind>,
) -> CommandOutputCompactReport {
    let normalized = normalize_command_output(input);
    let detected_kind = match options.kind {
        CommandOutputKind::Auto => detect_command_output_kind_with_hint(&normalized, kind_hint),
        explicit => explicit,
    };
    let specialized_output = compact_ci_failure_log_output(&normalized, options)
        .or_else(|| compact_watcher_delta_output(&normalized, detected_kind, options));
    let structured_json_output = (specialized_output.is_none()
        && command_output_kind_allows_structured_json_compaction(detected_kind))
    .then(|| compact_structured_json_output(&normalized, options))
    .flatten();
    let output = specialized_output
        .or(structured_json_output)
        .unwrap_or_else(|| match detected_kind {
            CommandOutputKind::Auto => smart_truncate_command_output(&normalized, options),
            CommandOutputKind::GitStatus => compact_git_status_output(&normalized, options),
            CommandOutputKind::GitDiff => compact_git_diff_output(&normalized, options),
            CommandOutputKind::RustDiagnostics => {
                compact_rust_diagnostic_output(&normalized, options)
            }
            CommandOutputKind::Diagnostics => compact_diagnostic_output(&normalized, options),
            CommandOutputKind::GitLog => compact_git_log_stat_output(&normalized, options),
            CommandOutputKind::Search => compact_search_output(&normalized, options),
            CommandOutputKind::FileList => compact_file_list_output(&normalized, options),
            CommandOutputKind::LogStream => compact_log_stream_output(&normalized, options),
            CommandOutputKind::NoisySuccess => compact_noisy_success_output(&normalized, options),
            CommandOutputKind::Plain => smart_truncate_command_output(&normalized, options),
        });
    let output = canonicalize_compacted_command_paths(&normalized, &output, detected_kind);

    let original_lines = count_text_lines(&normalized);
    let compacted_lines = count_text_lines(&output);
    CommandOutputCompactReport {
        requested_kind: options.kind,
        detected_kind,
        original_lines,
        compacted_lines,
        estimated_tokens_before: estimate_context_tokens(
            normalized.chars().count(),
            normalized.split_whitespace().count(),
        ),
        estimated_tokens_after: estimate_context_tokens(
            output.chars().count(),
            output.split_whitespace().count(),
        ),
        output,
    }
}

fn command_output_kind_allows_structured_json_compaction(kind: CommandOutputKind) -> bool {
    matches!(
        kind,
        CommandOutputKind::Auto | CommandOutputKind::Plain | CommandOutputKind::Search
    )
}

pub fn compact_command_output_with_intent_terms(
    input: &str,
    options: &CommandOutputCompactOptions,
    intent_terms: &[String],
) -> CommandOutputCompactReport {
    compact_command_output_with_intent_options(
        input,
        &CommandOutputIntentCompactOptions::new(options.clone(), intent_terms.to_vec()),
    )
}

pub fn compact_command_output_with_intent_options(
    input: &str,
    options: &CommandOutputIntentCompactOptions,
) -> CommandOutputCompactReport {
    let intent_terms = normalize_intent_terms_with_prompt_expansion(&options.intent_terms);
    if intent_terms.is_empty() {
        return compact_command_output_with_options_and_kind_hint(
            input,
            &options.base,
            options.kind_hint,
        );
    }

    let normalized = normalize_command_output(input);
    let mut report =
        compact_command_output_with_options_and_kind_hint(input, &options.base, options.kind_hint);
    let output = compact_command_output_for_intent(
        &normalized,
        &report.output,
        report.detected_kind,
        &options.base,
        &intent_terms,
    );
    let output = ensure_no_critical_signal_loss_for_intent(&normalized, &output, &options.base);
    let output = canonicalize_compacted_command_paths(&normalized, &output, report.detected_kind);

    report.compacted_lines = count_text_lines(&output);
    report.estimated_tokens_after =
        estimate_context_tokens(output.chars().count(), output.split_whitespace().count());
    report.output = output;
    report
}

fn compact_rust_diagnostic_output(input: &str, options: &CommandOutputCompactOptions) -> String {
    let lines = command_lines(input);
    if lines.is_empty() {
        return String::new();
    }

    let mut summary = RustDiagnosticSummary::default();
    let mut noise_counts = BTreeMap::<String, usize>::new();
    let mut key_lines = Vec::<String>::new();
    let mut blocks = Vec::<RustCriticalBlock>::new();
    let block_limit = rust_block_line_limit(options);
    let block_budget = options.max_lines.max(24).saturating_div(3).max(4);
    let mut used_block_lines = 0usize;
    let mut omitted_blocks = 0usize;
    let mut index = 0usize;

    while index < lines.len() {
        let line = lines[index];
        if let Some(label) = rust_noise_label(line) {
            *noise_counts.entry(label.to_string()).or_default() += 1;
            if is_rust_success_summary_line(line) {
                push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
            }
            index += 1;
            continue;
        }

        if let Some(severity) = rust_diagnostic_severity(line) {
            summary.record_diagnostic(severity, line);
            let (block, next_index) = collect_rust_diagnostic_block(&lines, index, block_limit);
            summary.record_block_signals(&block);
            if used_block_lines.saturating_add(block.lines.len()) <= block_budget
                || blocks.is_empty()
            {
                used_block_lines = used_block_lines.saturating_add(block.lines.len());
                blocks.push(block);
            } else {
                omitted_blocks += 1;
            }
            index = next_index;
            continue;
        }

        if let Some(test_name) = rust_failed_test_name(line) {
            summary.record_failed_test(test_name);
            push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
            let (block, next_index) = collect_rust_failure_block(&lines, index, block_limit);
            summary.record_block_signals(&block);
            if used_block_lines.saturating_add(block.lines.len()) <= block_budget
                || blocks.is_empty()
            {
                used_block_lines = used_block_lines.saturating_add(block.lines.len());
                blocks.push(block);
            } else {
                omitted_blocks += 1;
            }
            index = next_index;
            continue;
        }

        if let Some(test_name) = rust_failure_separator_name(line) {
            summary.record_failed_test(test_name);
            let (block, next_index) = collect_rust_failure_block(&lines, index, block_limit);
            summary.record_block_signals(&block);
            if used_block_lines.saturating_add(block.lines.len()) <= block_budget
                || blocks.is_empty()
            {
                used_block_lines = used_block_lines.saturating_add(block.lines.len());
                blocks.push(block);
            } else {
                omitted_blocks += 1;
            }
            index = next_index;
            continue;
        }

        if is_rust_panic_line(line) || is_rust_backtrace_start(line) {
            let (block, next_index) = collect_rust_failure_block(&lines, index, block_limit);
            summary.record_block_signals(&block);
            if used_block_lines.saturating_add(block.lines.len()) <= block_budget
                || blocks.is_empty()
            {
                used_block_lines = used_block_lines.saturating_add(block.lines.len());
                blocks.push(block);
            } else {
                omitted_blocks += 1;
            }
            index = next_index;
            continue;
        }

        if is_rust_exit_status_line(line) {
            summary.record_exit_status(line);
            push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
            index += 1;
            continue;
        }

        if is_rust_location_line(line) {
            summary.record_location(line);
            push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
            index += 1;
            continue;
        }

        if is_rust_failure_summary_line(line) {
            push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
            index += 1;
            continue;
        }

        index += 1;
    }

    if summary.is_empty() && noise_counts.is_empty() && key_lines.is_empty() && blocks.is_empty() {
        return smart_truncate_command_output(input, options);
    }

    let mut output = Vec::new();
    output.push(format!(
        "sum: rust errors={}, warnings={}, failed_tests={}, panics={}, exit_statuses={}, noisy={}",
        summary.errors,
        summary.warnings,
        summary.failed_tests.len(),
        summary.panics,
        summary.exit_statuses.len(),
        noise_counts.values().sum::<usize>(),
    ));
    push_labeled_lines(
        &mut output,
        "root causes",
        &summary.root_causes,
        options.max_lines.max(24).saturating_div(6).max(3),
    );
    push_labeled_lines(
        &mut output,
        "diagnostics",
        &summary.diagnostic_headers,
        options.max_lines.max(24).saturating_div(4).max(4),
    );
    push_labeled_lines(
        &mut output,
        "locations",
        &summary.locations,
        options.max_lines.max(24).saturating_div(5).max(4),
    );
    push_labeled_lines(
        &mut output,
        "failed tests",
        &summary.failed_tests,
        options.max_lines.max(24).saturating_div(5).max(4),
    );
    push_labeled_lines(
        &mut output,
        "exit statuses",
        &summary.exit_statuses,
        options.max_lines.max(24).saturating_div(6).max(3),
    );
    push_labeled_lines(
        &mut output,
        "key lines",
        &key_lines,
        options.max_lines.max(24).saturating_div(6).max(3),
    );

    if !blocks.is_empty() {
        output.push("critical blocks:".to_string());
        for block in blocks {
            output.push(format!("-- {} --", block.label));
            for line in block.lines {
                output.push(truncate_command_line(&line, options.max_line_chars));
            }
        }
    }
    if omitted_blocks > 0 {
        output.push(format!(
            "[... omitted {omitted_blocks} additional critical blocks ...]"
        ));
    }
    if !noise_counts.is_empty() {
        output.push(format_count_map("noise", &noise_counts, 10));
    }

    finalize_compacted_command_output(CommandOutputKind::RustDiagnostics, input, output, options)
}

fn compact_diagnostic_output(input: &str, options: &CommandOutputCompactOptions) -> String {
    let lines = command_lines(input);
    if lines.is_empty() {
        return String::new();
    }

    let mut summary = CommandDiagnosticSummary::default();
    let mut noise_counts = BTreeMap::<String, usize>::new();
    let mut key_lines = Vec::<String>::new();
    let mut blocks = Vec::<CommandCriticalBlock>::new();
    let block_limit = diagnostic_block_line_limit(options);
    let block_budget = options.max_lines.max(24).saturating_div(3).max(6);
    let mut used_block_lines = 0usize;
    let mut omitted_blocks = 0usize;
    let mut index = 0usize;

    while index < lines.len() {
        let line = lines[index];
        if let Some(label) = diagnostic_noise_label(line) {
            *noise_counts.entry(label.to_string()).or_default() += 1;
            if is_diagnostic_key_line(line) {
                push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
            }
            index += 1;
            continue;
        }

        if is_diagnostic_block_start(line) {
            summary.record_line(line);
            let (block, next_index) = collect_diagnostic_block(&lines, index, block_limit);
            summary.record_block_signals(&block);
            if used_block_lines.saturating_add(block.lines.len()) <= block_budget
                || blocks.is_empty()
            {
                used_block_lines = used_block_lines.saturating_add(block.lines.len());
                blocks.push(block);
            } else {
                omitted_blocks += 1;
            }
            index = next_index;
            continue;
        }

        if is_critical_preserve_line(line) || count_file_location_signals(line) > 0 {
            summary.record_line(line);
            push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
            index += 1;
            continue;
        }

        if is_success_output_failure_signal_line(line)
            || is_success_output_warning_signal_line(line)
        {
            push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
            index += 1;
            continue;
        }
        index += 1;
    }

    if summary.is_empty() && noise_counts.is_empty() && key_lines.is_empty() && blocks.is_empty() {
        return smart_truncate_command_output(input, options);
    }

    let mut output = Vec::new();
    output.push(format!(
        "sum: diag errors={}, failed_tests={}, locations={}, stack={}, exit_statuses={}, noisy={}",
        summary.errors,
        summary.failed_tests.len(),
        summary.locations.len(),
        summary.stack_markers,
        summary.exit_statuses.len(),
        noise_counts.values().sum::<usize>(),
    ));
    push_labeled_lines(
        &mut output,
        "root causes",
        &summary.root_causes,
        options.max_lines.max(24).saturating_div(6).max(3),
    );
    push_labeled_lines(
        &mut output,
        "diagnostics",
        &summary.diagnostic_headers,
        options.max_lines.max(24).saturating_div(4).max(4),
    );
    push_labeled_lines(
        &mut output,
        "locations",
        &summary.locations,
        options.max_lines.max(24).saturating_div(5).max(4),
    );
    push_labeled_lines(
        &mut output,
        "failed tests",
        &summary.failed_tests,
        options.max_lines.max(24).saturating_div(5).max(4),
    );
    push_labeled_lines(
        &mut output,
        "exit statuses",
        &summary.exit_statuses,
        options.max_lines.max(24).saturating_div(6).max(3),
    );
    push_labeled_lines(
        &mut output,
        "key lines",
        &key_lines,
        options.max_lines.max(24).saturating_div(6).max(3),
    );

    if !blocks.is_empty() {
        output.push("critical blocks:".to_string());
        for block in blocks {
            output.push(format!("-- {} --", block.label));
            for line in block.lines {
                output.push(truncate_command_line(&line, options.max_line_chars));
            }
        }
    }
    if omitted_blocks > 0 {
        output.push(format!(
            "[... omitted {omitted_blocks} additional critical blocks ...]"
        ));
    }
    if !noise_counts.is_empty() {
        output.push(format_count_map("noise", &noise_counts, 10));
    }

    finalize_compacted_command_output(CommandOutputKind::Diagnostics, input, output, options)
}

fn compact_noisy_success_output(input: &str, options: &CommandOutputCompactOptions) -> String {
    let lines = command_lines(input);
    if lines.is_empty() {
        return String::new();
    }
    if !count_critical_signals(input).is_empty()
        || lines.iter().any(|line| {
            is_success_output_failure_signal_line(line)
                || is_success_output_warning_signal_line(line)
        })
    {
        return smart_truncate_command_output(input, options);
    }

    let mut noise_counts = BTreeMap::<String, usize>::new();
    let mut key_lines = Vec::<String>::new();
    let mut critical_lines = Vec::<String>::new();
    for line in lines {
        if let Some(label) = noisy_success_label(line) {
            *noise_counts.entry(label.to_string()).or_default() += 1;
            if is_noisy_success_key_line(line) {
                push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
            }
            continue;
        }
        if is_noisy_success_key_line(line) {
            push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
        }
        if is_critical_preserve_line(line) {
            push_unique_truncated_line(&mut critical_lines, line, options.max_line_chars);
        }
    }

    if noise_counts.is_empty() && key_lines.is_empty() && critical_lines.is_empty() {
        return smart_truncate_command_output(input, options);
    }

    let mut output = Vec::new();
    output.push(format!(
        "sum: success noisy={}, critical={}",
        noise_counts.values().sum::<usize>(),
        critical_lines.len(),
    ));
    if !noise_counts.is_empty() {
        output.push(format_count_map("noise", &noise_counts, 10));
    }
    push_labeled_lines(
        &mut output,
        "key lines",
        &key_lines,
        options.max_lines.max(24).saturating_div(3).max(4),
    );
    push_labeled_lines(
        &mut output,
        "critical lines",
        &critical_lines,
        options.max_lines.max(24).saturating_div(4).max(4),
    );

    finalize_compacted_command_output(CommandOutputKind::NoisySuccess, input, output, options)
}

fn compact_watcher_delta_output(
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

fn looks_like_rust_diagnostic_output(lines: &[&str]) -> bool {
    let mut strong_signals = 0usize;
    let mut cargo_noise_signals = 0usize;
    let mut location_signals = 0usize;
    let mut backtrace_signals = 0usize;
    let mut exit_signals = 0usize;
    let mut clippy_signals = 0usize;
    for line in lines {
        if rust_diagnostic_severity(line).is_some()
            || rust_failed_test_name(line).is_some()
            || rust_failure_separator_name(line).is_some()
            || is_rust_panic_line(line)
            || is_rust_failure_summary_line(line)
        {
            strong_signals += 1;
        }
        if is_rust_location_line(line) {
            location_signals += 1;
        }
        if is_rust_backtrace_start(line) {
            backtrace_signals += 1;
        }
        if is_rust_exit_status_line(line) {
            exit_signals += 1;
        }
        if rust_noise_label(line).is_some() {
            cargo_noise_signals += 1;
        }
        if line.contains("clippy::") || line.contains("cargo clippy") {
            clippy_signals += 1;
        }
    }

    if strong_signals > 0 {
        return strong_signals
            + cargo_noise_signals
            + location_signals
            + backtrace_signals
            + exit_signals
            + clippy_signals
            >= 2;
    }
    if backtrace_signals > 0 && location_signals > 0 {
        return true;
    }
    if exit_signals > 0 && (cargo_noise_signals > 0 || location_signals > 0) {
        return true;
    }
    cargo_noise_signals >= 4
}

fn looks_like_diagnostic_output(lines: &[&str]) -> bool {
    let mut strong_signals = 0usize;
    let mut location_signals = 0usize;
    let mut stack_signals = 0usize;
    let mut exit_signals = 0usize;
    let mut noise_signals = 0usize;

    for line in lines {
        if is_diagnostic_detection_start(line) {
            strong_signals += 1;
        }
        if count_file_location_signals(line) > 0 {
            location_signals += 1;
        }
        if is_stack_signal_line(line) {
            stack_signals += 1;
        }
        if is_rust_exit_status_line(line) {
            exit_signals += 1;
        }
        if diagnostic_noise_label(line).is_some() {
            noise_signals += 1;
        }
    }

    if lines
        .iter()
        .any(|line| is_junit_xml_failure_line(line) || is_eslint_diagnostic_line(line))
    {
        return true;
    }
    if strong_signals > 0 {
        return strong_signals + location_signals + stack_signals + exit_signals + noise_signals
            >= 2;
    }
    stack_signals > 0 && location_signals > 0
}

fn looks_like_noisy_success_output(lines: &[&str]) -> bool {
    let non_empty = lines.iter().filter(|line| !line.trim().is_empty()).count();
    if non_empty < 8 {
        return false;
    }
    if lines.iter().any(|line| {
        is_error_signal_line(line)
            || is_test_failure_signal_line(line)
            || is_success_output_failure_signal_line(line)
            || is_success_output_warning_signal_line(line)
    }) {
        return false;
    }

    let success_signals = lines
        .iter()
        .filter(|line| noisy_success_label(line).is_some())
        .count();
    let key_lines = lines
        .iter()
        .filter(|line| is_noisy_success_key_line(line))
        .count();
    success_signals >= 4 || (key_lines > 0 && success_signals >= 2)
}

fn rust_block_line_limit(options: &CommandOutputCompactOptions) -> usize {
    options.max_lines.max(24).saturating_div(5).clamp(8, 32)
}

fn diagnostic_block_line_limit(options: &CommandOutputCompactOptions) -> usize {
    options.max_lines.max(24).saturating_div(5).clamp(8, 32)
}

fn rust_noise_label(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();
    if is_success_output_failure_signal_line(trimmed)
        || is_success_output_warning_signal_line(trimmed)
        || is_error_signal_line(trimmed)
        || is_test_failure_signal_line(trimmed)
    {
        return None;
    }

    if trimmed.starts_with("Compiling ") {
        Some("compiling")
    } else if trimmed.starts_with("Checking ") {
        Some("checking")
    } else if trimmed.starts_with("Fresh ") {
        Some("fresh")
    } else if trimmed.starts_with("Documenting ") {
        Some("documenting")
    } else if trimmed.starts_with("Formatting ") {
        Some("formatting")
    } else if trimmed.starts_with("Fixing ") || trimmed.starts_with("Fixed ") {
        Some("cargo_fix")
    } else if trimmed.starts_with("Generated ") {
        Some("generated_docs")
    } else if trimmed.starts_with("Finished ") {
        Some("finished")
    } else if trimmed.starts_with("Running ") {
        Some("running_targets")
    } else if trimmed.starts_with("Doc-tests ") {
        Some("doc_tests")
    } else if trimmed.starts_with("running ") && trimmed.ends_with(" tests") {
        Some("running_tests")
    } else if trimmed.starts_with("test ") && trimmed.contains(" ... ok") {
        Some("passed_tests")
    } else if trimmed.starts_with("PASS [") {
        Some("nextest_pass")
    } else if trimmed.starts_with("Summary [") && trimmed.contains(" passed") {
        Some("nextest_summary")
    } else if trimmed.starts_with("test result: ok") {
        Some("test_result_ok")
    } else {
        None
    }
}

fn diagnostic_noise_label(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    if trimmed.starts_with("> ") {
        Some("npm_script")
    } else if lower.starts_with("collecting ") {
        Some("pytest_collecting")
    } else if lower.starts_with("collected ") && lower.contains(" item") {
        Some("pytest_collected")
    } else if is_pytest_progress_line(trimmed) {
        Some("pytest_progress")
    } else {
        noisy_success_label(line)
    }
}

fn noisy_success_label(line: &str) -> Option<&'static str> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if is_success_output_failure_signal_line(trimmed)
        && !is_diagnostic_failure_summary_line(trimmed)
        || is_success_output_warning_signal_line(trimmed)
    {
        return None;
    }

    if is_coverage_noise_line(trimmed, &lower) {
        Some("coverage")
    } else if is_gradle_test_success_line(trimmed, &lower) {
        Some("gradle_test")
    } else if is_maven_test_success_line(trimmed, &lower) {
        Some("maven_test")
    } else if is_package_install_success_line(&lower) {
        Some("package_install")
    } else if is_docker_buildx_success_line(&lower) {
        Some("docker_buildx")
    } else if is_bazel_test_success_line(&lower) {
        Some("bazel_test")
    } else if is_junit_xml_success_line(trimmed, &lower) {
        Some("junit_xml")
    } else if is_swift_test_success_line(&lower) {
        Some("swift_test")
    } else if is_playwright_success_line(trimmed, &lower) {
        Some("playwright")
    } else if is_biome_success_summary_line(&lower) {
        Some("biome_summary")
    } else if is_oxlint_success_summary_line(&lower) {
        Some("oxlint_summary")
    } else if let Some(label) = rust_noise_label(line) {
        Some(label)
    } else if is_typescript_success_line(trimmed, &lower) {
        Some("typecheck_summary")
    } else if is_vite_success_line(trimmed, &lower) {
        Some("vite")
    } else if is_next_success_line(trimmed, &lower) {
        Some("next")
    } else if is_dot_reporter_success_line(trimmed) {
        Some("dot_progress")
    } else if is_bun_test_success_line(trimmed, &lower) {
        Some("bun_test")
    } else if is_cypress_success_line(trimmed, &lower) {
        Some("cypress")
    } else if is_zig_test_success_line(&lower) {
        Some("zig_test")
    } else if trimmed.starts_with("PASS ") {
        Some("passed_suites")
    } else if lower.starts_with("ok ") && lower.split_whitespace().count() >= 2 {
        Some("go_test_ok")
    } else if lower.starts_with("? ") && lower.contains("[no test files]") {
        Some("go_test_no_files")
    } else if lower.starts_with("=== run ") {
        Some("go_test_run")
    } else if lower.starts_with("=== pause ") {
        Some("go_test_pause")
    } else if lower.starts_with("=== cont ") {
        Some("go_test_cont")
    } else if lower.starts_with("--- pass: ") {
        Some("go_test_pass")
    } else if lower.starts_with("--- skip: ") {
        Some("go_test_skip")
    } else if trimmed == "PASS" {
        Some("go_test_pass_summary")
    } else if lower.starts_with("test suites:") && lower.contains("passed") {
        Some("test_suites")
    } else if lower.starts_with("tests:") && lower.contains("passed") {
        Some("test_cases")
    } else if lower.starts_with("summary [") && lower.contains(" passed") {
        Some("nextest_summary")
    } else if trimmed.starts_with("PASS [") {
        Some("nextest_pass")
    } else if lower.starts_with("snapshots:")
        && (lower.contains("passed") || lower.contains("0 total"))
    {
        Some("snapshots")
    } else if lower.starts_with("test files") && lower.contains("passed") {
        Some("test_files")
    } else if lower.starts_with("duration") {
        Some("test_duration")
    } else if lower.starts_with("time:") {
        Some("test_time")
    } else if lower.starts_with("ran all test suites") {
        Some("test_runner_summary")
    } else if lower.starts_with("done in ") {
        Some("done")
    } else if lower.starts_with("build successful")
        || lower.starts_with("build success")
        || lower.starts_with("[info] build success")
        || lower.contains(" build success")
    {
        Some("build_success")
    } else if lower.starts_with("[info] --- ") || lower.starts_with("> task ") {
        Some("build_steps")
    } else if lower.starts_with("info: analyzed target")
        || lower.starts_with("info: found ")
        || lower.starts_with("info: elapsed time:")
    {
        Some("bazel_steps")
    } else if (lower.starts_with("target ") && lower.contains("up-to-date"))
        || lower.starts_with("info: build completed successfully")
    {
        Some("bazel_summary")
    } else if lower.contains("successfully ran target")
        || lower.contains("successfully ran targets")
        || lower.starts_with("nx successfully ran")
    {
        Some("nx_summary")
    } else if lower.starts_with("tasks:") && lower.contains("successful") {
        Some("turbo_summary")
    } else if lower.contains("actionable tasks:") || lower.contains("actionable task:") {
        Some("gradle_tasks")
    } else if lower.starts_with("[info] total time:")
        || lower.starts_with("[info] finished at:")
        || lower.starts_with("[info] tests run:")
    {
        Some("maven_summary")
    } else if lower.starts_with("=> ")
        || lower.starts_with("=>=> ")
        || lower.starts_with("#") && (lower.contains(" done ") || lower.ends_with(" done"))
    {
        Some("docker_steps")
    } else if lower.starts_with("[+] running ")
        || (lower.starts_with("container ") && docker_compose_success_state(&lower))
        || (lower.starts_with("network ") && lower.contains("created"))
        || (lower.starts_with("volume ") && lower.contains("created"))
    {
        Some("docker_compose")
    } else if lower.starts_with("successfully built ")
        || lower.starts_with("successfully tagged ")
        || lower.contains("writing image sha256:")
        || lower.contains("naming to ")
    {
        Some("docker_summary")
    } else if lower.starts_with("running ") && lower.contains(" tests using ") {
        Some("playwright_running")
    } else if lower.contains(" passed (") && lower.chars().any(|ch| ch.is_ascii_digit()) {
        Some("test_summary")
    } else if lower.starts_with("added ") && lower.contains(" package") {
        Some("packages_added")
    } else if lower.starts_with("audited ") && lower.contains(" package") {
        Some("packages_audited")
    } else if lower.starts_with("updating crates.io index") {
        Some("cargo_index")
    } else if lower.starts_with("locking ") && lower.contains(" package") {
        Some("cargo_lock")
    } else if lower.starts_with("downloading crates") || lower.starts_with("downloaded ") {
        Some("cargo_download")
    } else if lower.starts_with("packages: ") || lower.starts_with("progress: resolved") {
        Some("package_progress")
    } else if lower.starts_with("lockfile is up to date") || lower.starts_with("already up to date")
    {
        Some("packages_up_to_date")
    } else if lower.starts_with("requirement already satisfied")
        || lower.starts_with("successfully installed")
        || lower.starts_with("installing collected packages")
        || (lower.starts_with("resolved ") && lower.contains(" package"))
        || (lower.starts_with("prepared ") && lower.contains(" package"))
        || (lower.starts_with("installed ") && lower.contains(" package"))
    {
        Some("python_packages")
    } else if lower == "up to date" || lower.starts_with("up to date in ") {
        Some("packages_up_to_date")
    } else if lower.starts_with("found 0 vulnerabilities") {
        Some("vulnerability_summary")
    } else if lower.starts_with("all files pass")
        || lower.contains("all matched files use prettier code style")
        || lower.contains("eslint found no problems")
        || lower.starts_with("all checks passed")
    {
        Some("formatter_summary")
    } else if lower.starts_with("success: no issues found")
        || lower.starts_with("found 0 errors")
        || lower.starts_with("found 0 warnings")
        || lower.starts_with("found 0 issues")
    {
        Some("typecheck_summary")
    } else if lower.starts_with("built in ") || lower.contains(" built in ") {
        Some("build_summary")
    } else if lower.starts_with("compiled successfully") {
        Some("compile_summary")
    } else if (lower.starts_with("tests/") && lower.contains(" passed"))
        || (lower.contains("::test_") && lower.ends_with(" passed"))
        || is_pytest_success_summary_line(&lower)
    {
        Some("passed_tests")
    } else if is_pytest_progress_line(trimmed) {
        Some("pytest_progress")
    } else {
        None
    }
}
