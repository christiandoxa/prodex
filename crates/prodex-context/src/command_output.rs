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
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

mod git_search;
mod intent;
mod structured_json;
mod success;

pub use intent::extract_intent_terms_from_prompt;
pub use success::compact_successful_command_output_with_options;

use git_search::{
    collect_file_list_entries, collect_search_output_matches, compact_file_list_output,
    compact_git_diff_output, compact_git_diff_output_with_intent, compact_git_log_stat_output,
    compact_git_status_output, compact_search_output,
};
use intent::{
    compact_command_output_for_intent, ensure_no_critical_signal_loss_for_intent,
    intent_line_matches, normalize_intent_terms_with_prompt_expansion, score_intent_text,
};
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

#[derive(Debug, Clone)]
struct CommandPathAlias {
    alias: String,
    prefix: String,
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

fn compact_log_stream_output(input: &str, options: &CommandOutputCompactOptions) -> String {
    let lines = command_lines(input);
    if lines.is_empty() {
        return String::new();
    }

    let level_counts = count_log_stream_levels(&lines);
    let preserved = collect_log_stream_preserved_lines(&lines, options);
    let non_empty = lines.iter().filter(|line| !line.trim().is_empty()).count();
    let mut output = Vec::<String>::new();
    output.push(format!("pcs: logs ({}->sum)", count_text_lines(input)));
    output.push(format!(
        "sum: logs lines={}, non_empty={}, preserved={}",
        lines.len(),
        non_empty,
        preserved.len()
    ));
    if !level_counts.is_empty() {
        output.push(format_count_map("levels", &level_counts, 10));
    }
    if !preserved.is_empty() {
        output.push("preserved:".to_string());
        for line in preserved {
            output.push(line);
        }
    }

    let text = lines_to_text(output);
    if critical_signal_self_check(input, &text).passed() {
        text
    } else {
        smart_truncate_command_output(input, options)
    }
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

fn compact_ci_failure_log_output(
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

fn looks_like_log_stream_output(lines: &[&str]) -> bool {
    let non_empty = lines.iter().filter(|line| !line.trim().is_empty()).count();
    if non_empty < 8 {
        return false;
    }
    let counts = count_log_stream_levels(lines);
    let log_level_lines = counts.values().sum::<usize>();
    let routine = ["info", "debug", "trace"]
        .iter()
        .map(|level| counts.get(*level).copied().unwrap_or_default())
        .sum::<usize>();
    let critical = ["warn", "error", "fatal"]
        .iter()
        .map(|level| counts.get(*level).copied().unwrap_or_default())
        .sum::<usize>();
    log_level_lines.saturating_mul(2) >= non_empty && (routine >= 4 || critical > 0)
}

fn count_log_stream_levels(lines: &[&str]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::<String, usize>::new();
    for line in lines {
        if let Some(level) = log_stream_level_label(line) {
            *counts.entry(level.to_string()).or_default() += 1;
        }
    }
    counts
}

fn collect_log_stream_preserved_lines(
    lines: &[&str],
    options: &CommandOutputCompactOptions,
) -> Vec<String> {
    let mut preserved = Vec::<String>::new();
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index];
        if !is_log_stream_preserve_line(line) {
            index += 1;
            continue;
        }

        preserved.push(line.to_string());
        let mut next = index + 1;
        let mut context = Vec::<&str>::new();
        while next < lines.len() && is_log_stream_stack_context_line(lines[next]) {
            context.push(lines[next]);
            next += 1;
        }
        push_log_stream_stack_context_lines(&mut preserved, &context, options);
        index = next;
    }
    preserved
}

fn push_log_stream_stack_context_lines(
    output: &mut Vec<String>,
    context: &[&str],
    options: &CommandOutputCompactOptions,
) {
    if context.is_empty() {
        return;
    }

    let limit = log_stream_stack_context_line_limit(options);
    if context.len() <= limit {
        output.extend(context.iter().map(|line| (*line).to_string()));
        return;
    }

    let mut selected = BTreeSet::<usize>::new();
    for (index, line) in context.iter().enumerate() {
        if is_critical_preserve_line(line) {
            selected.insert(index);
        }
    }

    let remaining = limit.saturating_sub(selected.len());
    let head = remaining.div_ceil(2);
    let tail = remaining.saturating_sub(head);
    for index in 0..head.min(context.len()) {
        selected.insert(index);
    }
    for index in context.len().saturating_sub(tail)..context.len() {
        selected.insert(index);
    }

    let mut omitted = 0usize;
    for (index, line) in context.iter().enumerate() {
        if selected.contains(&index) {
            if omitted > 0 {
                output.push(format!(
                    "[... omitted {omitted} non-critical stack context lines ...]"
                ));
                omitted = 0;
            }
            output.push((*line).to_string());
        } else {
            omitted += 1;
        }
    }
    if omitted > 0 {
        output.push(format!(
            "[... omitted {omitted} non-critical stack context lines ...]"
        ));
    }
}

fn log_stream_stack_context_line_limit(options: &CommandOutputCompactOptions) -> usize {
    options.max_lines.max(18).saturating_div(3).clamp(6, 18)
}

fn is_log_stream_preserve_line(line: &str) -> bool {
    matches!(
        log_stream_level_label(line),
        Some("warn" | "error" | "fatal")
    ) || is_critical_preserve_line(line)
        || is_log_stream_critical_text_line(line)
}

fn is_log_stream_stack_context_line(line: &str) -> bool {
    if line.trim().is_empty() || log_stream_level_label(line).is_some() {
        return false;
    }
    let trimmed = line.trim_start();
    line.chars().next().is_some_and(char::is_whitespace)
        || trimmed.starts_with("at ")
        || trimmed.starts_with("File ")
        || trimmed.starts_with("Caused by:")
        || trimmed.starts_with("Traceback ")
        || trimmed.starts_with("Stack trace:")
        || trimmed.starts_with("stack trace:")
        || count_file_location_signals(line) > 0
        || is_exception_signal_line(line)
}

fn is_log_stream_critical_text_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    lower.contains("critical") || lower.contains("panic") || lower.contains("exception")
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

fn docker_compose_success_state(lower: &str) -> bool {
    lower.contains(" started")
        || lower.contains(" running")
        || lower.contains(" healthy")
        || lower.contains(" created")
        || lower.contains(" done")
        || lower.contains(" pulled")
}

fn is_dot_reporter_success_line(trimmed: &str) -> bool {
    trimmed.len() >= 4 && trimmed.chars().all(|ch| ch == '.')
}

fn is_bun_test_success_line(trimmed: &str, lower: &str) -> bool {
    lower.starts_with("bun test v")
        || lower.starts_with("(pass) ")
        || lower.starts_with("ran ") && lower.contains(" tests across ") && lower.contains(" file")
        || zero_count_summary_line(lower, "fail")
        || is_count_word_line(lower, "pass")
        || lower.contains(" expect() call")
        || trimmed.ends_with(".test.ts:")
        || trimmed.ends_with(".test.tsx:")
        || trimmed.ends_with(".test.js:")
        || trimmed.ends_with(".test.jsx:")
}

fn is_swift_test_success_line(lower: &str) -> bool {
    lower.starts_with("build complete!")
        || (lower.starts_with("test suite ") || lower.contains(" test suite "))
            && lower.contains(" passed at ")
        || (lower.starts_with("test case ") || lower.contains(" test case "))
            && lower.contains(" passed (")
        || lower.starts_with("executed ")
            && lower.contains(" tests")
            && lower.contains("with 0 failures")
}

fn is_zig_test_success_line(lower: &str) -> bool {
    lower == "test"
        || lower.starts_with("run test")
        || lower.contains(" run test")
        || lower.contains(" zig test ") && lower.contains(" passed")
        || lower.contains(" steps succeeded")
        || lower.contains(" tests passed")
        || lower.starts_with("build summary:")
            && lower.contains("succeeded")
            && !has_nonzero_summary_count(lower, &["failed", "failures", "error", "errors"])
}

fn is_gradle_test_success_line(trimmed: &str, lower: &str) -> bool {
    lower.starts_with("> task ") && lower.contains(":test") && !lower.ends_with(" failed")
        || lower.contains(" > ") && (lower.ends_with(" passed") || lower.ends_with(" skipped"))
        || lower.starts_with("test run finished after")
        || lower.starts_with("[") && lower.contains(" tests successful")
        || lower.starts_with("[") && lower.contains(" tests skipped")
        || lower.ends_with(" tests completed")
        || lower.contains(" tests completed, 0 failed")
        || trimmed == "BUILD SUCCESSFUL"
}

fn is_maven_test_success_line(_trimmed: &str, lower: &str) -> bool {
    let info_body = lower.strip_prefix("[info]").map(str::trim_start);
    lower.starts_with("[info] running ")
        || lower.starts_with("[info] results:")
        || lower.starts_with("[info] surefire report directory:")
        || lower.starts_with("[info] tests run:")
            && !has_nonzero_summary_count(lower, &["failures", "errors"])
        || lower.starts_with("[info] t e s t s")
        || info_body.is_some_and(|body| {
            body.len() >= 8 && body.chars().all(|ch| ch == '-' || ch.is_ascii_whitespace())
        })
}

fn is_package_install_success_line(lower: &str) -> bool {
    lower.starts_with("yarn install v")
        || lower.starts_with("[1/4] resolving packages")
        || lower.starts_with("[2/4] fetching packages")
        || lower.starts_with("[3/4] linking dependencies")
        || lower.starts_with("[4/4] building fresh packages")
        || lower.starts_with("success saved lockfile")
        || lower.starts_with("success already up-to-date")
        || lower.starts_with("saved lockfile")
        || lower.starts_with("bun install v")
        || lower.starts_with("resolved, downloaded and extracted")
        || lower.contains(" packages installed")
        || lower.starts_with("scope: all ") && lower.contains("workspace project")
        || lower.starts_with("done in ") && lower.contains(" using pnpm ")
}

fn is_docker_buildx_success_line(lower: &str) -> bool {
    lower.starts_with('#')
        && (lower.contains(" building with ")
            || lower.contains(" transferring ")
            || lower.contains(" exporting ")
            || lower.contains(" importing cache manifest")
            || lower.contains(" resolving provenance")
            || lower.contains(" writing image sha256:")
            || lower.contains(" naming to ")
            || lower.contains(" pushing layers")
            || lower.contains(" pushing manifest")
            || lower.ends_with(" cached"))
}

fn is_bazel_test_success_line(lower: &str) -> bool {
    lower.starts_with("//") && (lower.contains(" passed in ") || lower.ends_with(" passed"))
        || lower.starts_with("executed ")
            && lower.contains(" out of ")
            && lower.contains(" tests")
            && (lower.contains(" tests pass") || lower.contains(" test passes"))
        || lower.starts_with("info: found ") && lower.contains(" test target")
        || lower.starts_with("info: ") && lower.contains(" processes:")
}

fn zero_count_summary_line(lower: &str, word: &str) -> bool {
    lower.match_indices(word).any(|(index, matched)| {
        count_after_word(lower, index + matched.len()).or_else(|| count_before_word(lower, index))
            == Some(0)
    })
}

fn is_count_word_line(lower: &str, expected_word: &str) -> bool {
    let mut words = lower.split_whitespace();
    let Some(count) = words.next() else {
        return false;
    };
    let Some(word) = words.next() else {
        return false;
    };
    words.next().is_none() && count.chars().all(|ch| ch.is_ascii_digit()) && word == expected_word
}

fn is_typescript_success_line(trimmed: &str, lower: &str) -> bool {
    lower.starts_with("project '") && lower.contains(" is up to date")
        || lower.starts_with("building project '")
        || lower.starts_with("updating unchanged output timestamps")
        || lower.starts_with("projects in this build:")
        || looks_like_typescript_project_line(trimmed)
}

fn looks_like_typescript_project_line(trimmed: &str) -> bool {
    let trimmed = trimmed.trim_matches(|ch| matches!(ch, '\'' | '"' | '`' | ',' | ';'));
    trimmed.ends_with("tsconfig.json") || trimmed.ends_with("tsconfig.tsbuildinfo")
}

fn is_vite_success_line(trimmed: &str, lower: &str) -> bool {
    lower.starts_with("vite ") && lower.contains(" building for production")
        || lower == "transforming..."
        || lower == "rendering chunks..."
        || lower == "computing gzip size..."
        || lower.contains(" modules transformed")
        || lower.starts_with("dist/") && (lower.contains(" kb") || lower.contains("gzip:"))
        || trimmed.starts_with('✓') && lower.contains("built in ")
}

fn is_next_success_line(trimmed: &str, lower: &str) -> bool {
    lower.starts_with("▲ next.js")
        || lower.starts_with("next.js ")
        || lower.starts_with("creating an optimized production build")
        || lower.starts_with("compiled successfully")
        || lower.contains("compiled successfully")
        || lower.starts_with("linting and checking validity of types")
        || lower.starts_with("collecting page data")
        || lower.starts_with("generating static pages")
        || lower.starts_with("finalizing page optimization")
        || lower.starts_with("collecting build traces")
        || lower.starts_with("route ") && lower.contains("first load js")
        || lower.starts_with("+ first load js")
        || lower.contains("first load js shared by all")
        || is_next_route_table_line(trimmed, lower)
}

fn is_next_route_table_line(trimmed: &str, lower: &str) -> bool {
    matches!(
        trimmed.chars().next(),
        Some('┌' | '├' | '└' | '│' | '○' | '●' | 'ƒ' | '+')
    ) && (lower.contains(" kb") || lower.contains("first load js") || lower.contains("static"))
}

fn is_coverage_noise_line(trimmed: &str, lower: &str) -> bool {
    if lower.starts_with("coverage summary")
        || lower.starts_with("all files")
        || lower.starts_with("statements")
        || lower.starts_with("branches")
        || lower.starts_with("functions")
        || lower.starts_with("lines")
        || lower.contains(" coverage: platform ")
        || lower.starts_with("name ") && lower.contains(" stmts ") && lower.contains(" cover")
        || lower.starts_with("total ") && lower.contains('%')
        || lower.starts_with("required test coverage") && lower.contains(" reached")
        || lower.starts_with("coverage html written")
        || lower.starts_with("coverage xml written")
        || lower.starts_with("coverage json written")
    {
        return true;
    }
    if looks_like_pytest_coverage_table_row(trimmed, lower) {
        return true;
    }
    trimmed.contains('|')
        && (lower.contains("% stmts")
            || lower.contains("% branch")
            || lower.contains("% funcs")
            || lower.contains("% lines")
            || lower.contains("uncovered line"))
}

fn looks_like_pytest_coverage_table_row(trimmed: &str, lower: &str) -> bool {
    let mut columns = lower.split_whitespace();
    let Some(path) = columns.next() else {
        return false;
    };
    looks_like_location_path(path)
        && lower.contains('%')
        && trimmed
            .split_whitespace()
            .skip(1)
            .filter(|column| {
                column
                    .trim_end_matches('%')
                    .chars()
                    .all(|ch| ch.is_ascii_digit() || ch == '.')
            })
            .count()
            >= 3
}

fn is_junit_xml_success_line(trimmed: &str, lower: &str) -> bool {
    (trimmed.starts_with("<testsuite") || trimmed.starts_with("<testsuites"))
        && lower.contains("tests=")
        && (lower.contains("failures=\"0\"")
            || lower.contains("failures='0'")
            || lower.contains("failures=0"))
        && (lower.contains("errors=\"0\"")
            || lower.contains("errors='0'")
            || lower.contains("errors=0"))
}

pub(crate) fn is_junit_xml_failure_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    trimmed.starts_with("<failure")
        || trimmed.starts_with("<error")
        || ((trimmed.starts_with("<testsuite") || trimmed.starts_with("<testsuites"))
            && (xmlish_nonzero_attr(&lower, "failures") || xmlish_nonzero_attr(&lower, "errors")))
}

fn xmlish_nonzero_attr(lower: &str, name: &str) -> bool {
    for needle in [
        format!("{name}=\""),
        format!("{name}='"),
        format!("{name}="),
    ] {
        if let Some(after) = lower.split(&needle).nth(1) {
            let digits = after
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            if matches!(digits.parse::<usize>(), Ok(count) if count > 0) {
                return true;
            }
        }
    }
    false
}

fn is_playwright_success_line(trimmed: &str, lower: &str) -> bool {
    (lower.starts_with("running ") && lower.contains(" tests using "))
        || (lower.contains(" passed (") && lower.chars().any(|ch| ch.is_ascii_digit()))
        || lower.starts_with("slow test file:")
        || trimmed.starts_with('✓')
}

fn is_cypress_success_line(trimmed: &str, lower: &str) -> bool {
    lower.contains("all specs passed")
        || lower.starts_with("spec")
        || lower.starts_with("tests")
        || lower.starts_with("passing")
        || trimmed.starts_with('✔')
}

fn is_biome_success_summary_line(lower: &str) -> bool {
    (lower.starts_with("checked ")
        || lower.starts_with("formatted ")
        || lower.starts_with("linted "))
        && lower.contains(" file")
        && lower.contains(" in ")
        && (lower.contains("no fixes applied")
            || lower.contains("fixed ")
            || lower.contains("no issues found"))
        || lower == "no fixes applied."
        || lower.starts_with("fixed ") && lower.contains(" file")
}

fn is_oxlint_success_summary_line(lower: &str) -> bool {
    lower.starts_with("finished in ")
        && lower.contains(" on ")
        && lower.contains(" file")
        && (lower.contains("0 warning")
            || lower.contains("0 error")
            || !lower.contains("warning") && !lower.contains("error"))
}

fn is_pytest_progress_line(line: &str) -> bool {
    let trimmed = line.trim();
    let Some(progress) = trimmed.split_whitespace().next() else {
        return false;
    };
    progress.len() >= 2
        && progress
            .chars()
            .all(|ch| matches!(ch, '.' | 's' | 'S' | 'x' | 'X'))
        && progress.chars().any(|ch| ch == '.')
        && trimmed
            .get(progress.len()..)
            .map(str::trim)
            .is_some_and(|rest| {
                rest.is_empty()
                    || rest.starts_with('[') && rest.ends_with(']') && rest.contains('%')
            })
}

fn is_pytest_success_summary_line(lower: &str) -> bool {
    lower.contains(" passed")
        && lower.contains(" in ")
        && !lower.contains(" failed")
        && !lower.contains(" error")
        && !lower.contains("errors")
}

fn is_diagnostic_key_line(line: &str) -> bool {
    is_diagnostic_success_summary_line(line)
        || is_diagnostic_failure_summary_line(line)
        || is_noisy_success_key_line(line)
}

fn is_diagnostic_success_summary_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    lower.starts_with("test suites:")
        || lower.starts_with("tests:")
        || lower.starts_with("snapshots:")
        || lower.starts_with("test files")
        || lower.starts_with("ran all test suites")
        || lower.starts_with("found 0 vulnerabilities")
        || is_junit_xml_success_line(line.trim_start(), &lower)
        || lower.contains(" passed in ")
        || is_common_success_summary_line(&lower)
}

fn is_diagnostic_failure_summary_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    lower.starts_with("test suites:") && has_nonzero_summary_count(&lower, &["failed"])
        || lower.starts_with("tests:") && has_nonzero_summary_count(&lower, &["failed"])
        || lower.contains(" failed, ") && has_nonzero_summary_count(&lower, &["failed"])
        || lower.starts_with("failed ")
            && !has_zero_only_summary_count(&lower, &["failed", "failures"])
        || lower.starts_with("error summary")
            && !has_zero_only_summary_count(&lower, &["error", "errors"])
        || is_junit_xml_failure_line(line)
}

fn is_noisy_success_key_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    lower.starts_with("test suites:")
        || lower.starts_with("tests:")
        || lower.starts_with("snapshots:")
        || lower.starts_with("test files")
        || lower.starts_with("summary [") && lower.contains(" passed")
        || lower.starts_with("ran all test suites")
        || lower.starts_with("done in ")
        || lower.starts_with("added ")
        || lower.starts_with("audited ")
        || lower.starts_with("packages: ")
        || lower.starts_with("build successful")
        || lower.contains(" build success")
        || lower.starts_with("[info] build success")
        || lower.starts_with("info: build completed successfully")
        || (lower.starts_with("target ") && lower.contains("up-to-date"))
        || lower.contains("successfully ran target")
        || lower.contains("successfully ran targets")
        || (lower.starts_with("tasks:") && lower.contains("successful"))
        || lower.contains("actionable tasks:")
        || lower.contains("actionable task:")
        || is_gradle_test_success_line(line.trim_start(), &lower)
        || lower.starts_with("[info] tests run:")
        || is_maven_test_success_line(line.trim_start(), &lower)
        || lower.starts_with("successfully built ")
        || lower.starts_with("successfully tagged ")
        || lower.starts_with("[+] running ")
        || (lower.starts_with("container ") && docker_compose_success_state(&lower))
        || lower.contains("writing image sha256:")
        || lower.contains("naming to ")
        || is_docker_buildx_success_line(&lower)
        || lower.contains("all matched files use prettier code style")
        || lower.contains("eslint found no problems")
        || lower.starts_with("found 0 vulnerabilities")
        || lower == "up to date"
        || lower.starts_with("up to date in ")
        || lower.starts_with("lockfile is up to date")
        || lower.starts_with("already up to date")
        || is_package_install_success_line(&lower)
        || lower.starts_with("successfully installed")
        || (lower.starts_with("resolved ") && lower.contains(" package"))
        || (lower.starts_with("prepared ") && lower.contains(" package"))
        || (lower.starts_with("installed ") && lower.contains(" package"))
        || lower.starts_with("all files pass")
        || lower.starts_with("all checks passed")
        || lower.starts_with("success: no issues found")
        || lower.starts_with("found 0 errors")
        || lower.starts_with("found 0 issues")
        || lower.starts_with("built in ")
        || lower.contains(" passed in ")
        || is_bazel_test_success_line(&lower)
        || is_common_success_summary_line(&lower)
        || lower.starts_with("compiled successfully")
        || is_coverage_noise_line(line.trim_start(), &lower)
        || is_junit_xml_success_line(line.trim_start(), &lower)
        || is_playwright_success_line(line.trim_start(), &lower)
        || is_cypress_success_line(line.trim_start(), &lower)
}

fn is_common_success_summary_line(lower: &str) -> bool {
    lower.starts_with("build successful")
        || lower.contains(" build success")
        || lower.starts_with("[info] build success")
        || lower.contains("actionable tasks:")
        || lower.contains("actionable task:")
        || lower.starts_with("[info] tests run:")
        || lower.starts_with("successfully built ")
        || lower.starts_with("successfully tagged ")
        || lower.starts_with("info: build completed successfully")
        || lower.contains("successfully ran target")
        || lower.contains("successfully ran targets")
        || (lower.starts_with("tasks:") && lower.contains("successful"))
        || (lower.starts_with("summary [") && lower.contains(" passed"))
        || lower.starts_with("success: no issues found")
        || lower.starts_with("found 0 errors")
        || lower.starts_with("all checks passed")
        || lower.contains("all matched files use prettier code style")
        || lower.contains("eslint found no problems")
        || (lower.starts_with("test files") && lower.contains("passed"))
        || lower.contains(" passed (")
        || lower.contains("all specs passed")
}

fn is_success_output_failure_signal_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    lower.starts_with("build failure")
        || lower.starts_with("build failed")
        || lower.contains("build did not complete successfully")
        || lower.contains("build did not complete")
        || lower.starts_with("info: build failed")
        || lower.starts_with("failed:")
        || lower.starts_with("fail:")
        || lower.starts_with("--- fail:")
        || lower.starts_with("(fail) ")
        || lower.starts_with("failed tests:")
        || lower.starts_with("there were failing")
        || lower.starts_with("there were test failures")
        || lower.contains(" test failures")
        || lower.contains("tests failed")
        || lower.starts_with("type error")
        || lower.contains("failed to compile")
        || lower.contains("failed to load")
        || lower.contains("failed with")
        || lower.contains("execution failed")
        || lower.contains("failed to solve")
        || lower.contains("executor failed running")
        || lower.starts_with("> task ") && lower.ends_with(" failed")
        || lower.starts_with("//") && lower.contains(" failed")
        || lower.starts_with('#') && lower.contains(" error")
        || lower.starts_with("err_pnpm_")
        || lower.contains("required test coverage") && lower.contains("not reached")
        || is_nonzero_fail_count_line(&lower)
        || is_junit_xml_failure_line(line)
        || has_nonzero_summary_count(
            &lower,
            &["failed", "failures", "failing", "fails", "error", "errors"],
        )
}

fn is_nonzero_fail_count_line(lower: &str) -> bool {
    lower.match_indices("fail").any(|(index, matched)| {
        let count = count_after_word(lower, index + matched.len())
            .or_else(|| count_before_word(lower, index));
        count.is_some_and(|count| count > 0)
            && lower
                .split_whitespace()
                .all(|word| word.chars().all(|ch| ch.is_ascii_digit()) || word == "fail")
    })
}

fn is_success_output_warning_signal_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    if lower.is_empty() || has_zero_only_summary_count(&lower, &["warning", "warnings"]) {
        return false;
    }

    lower.starts_with("warning")
        || lower.starts_with("warn ")
        || lower.starts_with("warn:")
        || lower.starts_with("[warning]")
        || lower.starts_with("[warn]")
        || lower.starts_with("npm warn")
        || lower.starts_with("pnpm warn")
        || lower.starts_with("yarn warning")
        || lower.starts_with("bun warning")
        || has_nonzero_summary_count(&lower, &["warning", "warnings"])
        || lower.contains(" warning ")
        || lower.contains(" warnings")
        || lower.contains("with warnings")
        || lower.contains("compiled with warning")
        || lower.contains("compiled with warnings")
        || lower.contains("warning ts")
        || lower.contains(": warning ts")
        || lower.contains(" - warning ts")
}

fn has_nonzero_summary_count(lower: &str, words: &[&str]) -> bool {
    words.iter().any(|word| {
        lower.match_indices(word).any(|(index, matched)| {
            if let Some(count) = count_after_word(lower, index + matched.len()) {
                return count > 0;
            }
            count_before_word(lower, index).is_some_and(|count| count > 0)
        })
    })
}

pub(crate) fn has_zero_only_summary_count(lower: &str, words: &[&str]) -> bool {
    let mut saw_count = false;
    for word in words {
        for (index, matched) in lower.match_indices(word) {
            let count = count_after_word(lower, index + matched.len())
                .or_else(|| count_before_word(lower, index));
            let Some(count) = count else {
                continue;
            };
            saw_count = true;
            if count > 0 {
                return false;
            }
        }
    }
    saw_count
}

fn count_after_word(lower: &str, after_word_index: usize) -> Option<usize> {
    let after = lower.get(after_word_index..)?.trim_start();
    let after = if let Some(rest) = after.strip_prefix(':') {
        rest
    } else if let Some(rest) = after.strip_prefix('=') {
        rest
    } else {
        after.strip_prefix('(')?
    };
    let digits = after
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse::<usize>().ok())?
}

fn count_before_word(lower: &str, word_index: usize) -> Option<usize> {
    let before = lower
        .get(..word_index)?
        .trim_end_matches(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ':' | ';' | '('));
    let digits = before
        .chars()
        .rev()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse::<usize>().ok())?
}

fn is_rust_success_summary_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("Finished ")
        || trimmed.starts_with("test result: ok")
        || trimmed.starts_with("Summary [") && trimmed.contains(" passed")
}

pub(crate) fn rust_diagnostic_severity(line: &str) -> Option<RustDiagnosticSeverity> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("error[") || trimmed.starts_with("error:") {
        Some(RustDiagnosticSeverity::Error)
    } else if trimmed.starts_with("warning[") || trimmed.starts_with("warning:") {
        Some(RustDiagnosticSeverity::Warning)
    } else {
        rust_short_diagnostic_severity(trimmed)
    }
}

fn rust_short_diagnostic_severity(line: &str) -> Option<RustDiagnosticSeverity> {
    let lower = line.to_ascii_lowercase();
    for (needle, severity) in [
        (": error", RustDiagnosticSeverity::Error),
        (": warning", RustDiagnosticSeverity::Warning),
    ] {
        let Some(position) = lower.find(needle) else {
            continue;
        };
        if contains_rust_file_location(&line[..position]) {
            return Some(severity);
        }
    }
    None
}

pub(crate) fn rust_failed_test_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("test ")?;
    let (name, status) = rest.rsplit_once(" ... ")?;
    (status == "FAILED").then_some(name.trim())
}

pub(crate) fn rust_failure_separator_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix("---- ")?.strip_suffix(" ----")?.trim();
    let name = inner
        .strip_suffix(" stdout")
        .or_else(|| inner.strip_suffix(" stderr"))
        .unwrap_or(inner)
        .trim();
    (!name.is_empty()).then_some(name)
}

pub(crate) fn generic_failed_test_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if let Some(name) = trimmed.strip_prefix("FAIL ") {
        return non_empty_prefix(name);
    }
    if let Some(name) = trimmed.strip_prefix("FAILED ") {
        return non_empty_prefix(name);
    }
    if let Some((name, status)) = trimmed.rsplit_once(' ')
        && status == "FAILED"
        && (name.contains("::") || looks_like_location_path(name))
    {
        return non_empty_prefix(name);
    }
    if trimmed.starts_with("Test Suites:") && has_nonzero_summary_count(&lower, &["failed"]) {
        return Some(trimmed);
    }
    if trimmed.starts_with("Tests:") && has_nonzero_summary_count(&lower, &["failed"]) {
        return Some(trimmed);
    }
    if trimmed.contains(" failed, ") && has_nonzero_summary_count(&lower, &["failed"]) {
        return Some(trimmed);
    }
    if trimmed.starts_with("failed ") && !has_zero_only_summary_count(&lower, &["failed"]) {
        return Some(trimmed);
    }
    None
}

fn non_empty_prefix(input: &str) -> Option<&str> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let name = trimmed
        .split(" - ")
        .next()
        .unwrap_or(trimmed)
        .split(" (")
        .next()
        .unwrap_or(trimmed)
        .trim();
    (!name.is_empty()).then_some(name)
}

pub(crate) fn is_rust_panic_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.contains("panicked at ") || trimmed.contains("panicked at:")
}

pub(crate) fn is_rust_backtrace_start(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed == "stack backtrace:" || trimmed == "Backtrace:"
}

pub(crate) fn is_rust_exit_status_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("exit status")
        || lower.contains("exit code")
        || lower.contains("exit_status")
        || lower.contains("process didn't exit successfully")
}

fn is_rust_location_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("--> ")
        || trimmed.starts_with("::: ")
        || (trimmed.starts_with("at ") && contains_rust_file_location(trimmed))
        || contains_rust_file_location(trimmed)
}

fn contains_rust_file_location(line: &str) -> bool {
    let Some((_, after_rs)) = line.split_once(".rs:") else {
        return false;
    };
    let mut chars = after_rs.chars();
    let mut saw_digit = false;
    while let Some(ch) = chars.next() {
        if ch.is_ascii_digit() {
            saw_digit = true;
            continue;
        }
        if ch == ':' {
            return saw_digit && chars.next().is_some_and(|next| next.is_ascii_digit());
        }
        return saw_digit;
    }
    saw_digit
}

pub(crate) fn is_rust_failure_summary_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("test result: FAILED")
        || trimmed == "failures:"
        || trimmed.starts_with("failures:")
        || trimmed.starts_with("error: aborting")
}

fn is_diagnostic_block_start(line: &str) -> bool {
    is_typescript_diagnostic_line(line)
        || is_eslint_diagnostic_line(line)
        || is_junit_xml_failure_line(line)
        || is_exception_signal_line(line)
        || is_node_stack_error_line(line)
        || is_test_failure_signal_line(line)
        || is_stack_signal_line(line)
        || is_error_signal_line(line)
}

fn is_diagnostic_detection_start(line: &str) -> bool {
    is_typescript_diagnostic_line(line)
        || is_eslint_diagnostic_line(line)
        || is_junit_xml_failure_line(line)
        || is_exception_signal_line(line)
        || is_node_stack_error_line(line)
        || is_test_failure_signal_line(line)
        || is_stack_signal_line(line)
        || line
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("npm err!")
}

pub(crate) fn is_typescript_diagnostic_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    (lower.contains("error ts")
        || lower.contains("warning ts")
        || lower.contains(" - error ts")
        || lower.contains(": error ts")
        || lower.contains(" - warning ts")
        || lower.contains(": warning ts"))
        && count_file_location_signals(trimmed) > 0
}

pub(crate) fn is_eslint_diagnostic_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    count_file_location_signals(trimmed) > 0
        && (lower.contains("  error  ")
            || lower.contains("  warning  ")
            || lower.contains(": error ")
            || lower.contains(": warning ")
            || lower.contains(" eslint "))
}

pub(crate) fn is_exception_signal_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("E   ") {
        return true;
    }
    let Some((prefix, _)) = trimmed.split_once(':') else {
        return false;
    };
    let prefix = prefix.trim();
    if prefix.is_empty() || prefix.contains(' ') && !prefix.ends_with("Error") {
        return false;
    }
    matches!(
        prefix,
        "Error"
            | "AssertionError"
            | "ImportError"
            | "ModuleNotFoundError"
            | "NameError"
            | "RuntimeError"
            | "SyntaxError"
            | "TypeError"
            | "ValueError"
            | "ZeroDivisionError"
            | "ReferenceError"
            | "RangeError"
            | "URIError"
            | "EvalError"
    ) || prefix.ends_with("Error")
        || prefix.ends_with("Exception")
}

fn is_node_stack_error_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("at ") && count_file_location_signals(trimmed) > 0
}

fn collect_rust_diagnostic_block(
    lines: &[&str],
    start: usize,
    max_lines: usize,
) -> (RustCriticalBlock, usize) {
    let mut end = start + 1;
    while end < lines.len() {
        let line = lines[end];
        if rust_diagnostic_severity(line).is_some()
            || rust_failed_test_name(line).is_some()
            || rust_failure_separator_name(line).is_some()
        {
            break;
        }
        if rust_noise_label(line).is_some() && !is_rust_success_summary_line(line) {
            break;
        }
        if line.trim().is_empty()
            && end + 1 < lines.len()
            && rust_diagnostic_severity(lines[end + 1]).is_some()
        {
            break;
        }
        end += 1;
        if end.saturating_sub(start) >= max_lines.saturating_mul(4) {
            break;
        }
    }

    let label = format!("diagnostic: {}", rust_critical_label(lines[start]));
    (
        RustCriticalBlock {
            label,
            lines: compact_rust_block_lines(&lines[start..end], max_lines),
        },
        end,
    )
}

fn collect_rust_failure_block(
    lines: &[&str],
    start: usize,
    max_lines: usize,
) -> (RustCriticalBlock, usize) {
    let mut end = start + 1;
    while end < lines.len() {
        let line = lines[end];
        if end > start + 1
            && (rust_diagnostic_severity(line).is_some()
                || rust_failed_test_name(line).is_some()
                || rust_failure_separator_name(line).is_some())
        {
            break;
        }
        if rust_noise_label(line).is_some() && !is_rust_success_summary_line(line) {
            break;
        }
        end += 1;
        if end.saturating_sub(start) >= max_lines.saturating_mul(4) {
            break;
        }
    }

    let label = if let Some(test_name) =
        rust_failed_test_name(lines[start]).or_else(|| rust_failure_separator_name(lines[start]))
    {
        format!("failed test: {test_name}")
    } else if is_rust_backtrace_start(lines[start]) {
        "stack backtrace".to_string()
    } else {
        format!("panic/error: {}", rust_critical_label(lines[start]))
    };
    (
        RustCriticalBlock {
            label,
            lines: compact_rust_block_lines(&lines[start..end], max_lines),
        },
        end,
    )
}

fn collect_diagnostic_block(
    lines: &[&str],
    start: usize,
    max_lines: usize,
) -> (CommandCriticalBlock, usize) {
    let mut end = start + 1;
    while end < lines.len() {
        let line = lines[end];
        if end > start + 1 && is_diagnostic_block_start(line) {
            break;
        }
        if diagnostic_noise_label(line).is_some() && !is_diagnostic_key_line(line) {
            break;
        }
        end += 1;
        if end.saturating_sub(start) >= max_lines.saturating_mul(4) {
            break;
        }
    }

    (
        CommandCriticalBlock {
            label: diagnostic_critical_label(lines[start]),
            lines: compact_command_block_lines(&lines[start..end], max_lines),
        },
        end,
    )
}

fn compact_rust_block_lines(lines: &[&str], max_lines: usize) -> Vec<String> {
    compact_command_block_lines(lines, max_lines)
}

fn compact_command_block_lines(lines: &[&str], max_lines: usize) -> Vec<String> {
    if lines.len() <= max_lines {
        return lines.iter().map(|line| (*line).to_string()).collect();
    }

    let head = max_lines.saturating_mul(2).div_ceil(3).max(1);
    let tail = max_lines.saturating_sub(head).saturating_sub(1);
    let omitted = lines.len().saturating_sub(head + tail);
    let mut output = lines
        .iter()
        .take(head)
        .map(|line| (*line).to_string())
        .collect::<Vec<_>>();
    output.push(format!("[... omitted {omitted} lines inside block ...]"));
    if tail > 0 {
        output.extend(
            lines
                .iter()
                .skip(lines.len().saturating_sub(tail))
                .map(|line| (*line).to_string()),
        );
    }
    output
}

fn rust_critical_label(line: &str) -> String {
    truncate_command_line(line.trim(), 96)
}

fn diagnostic_critical_label(line: &str) -> String {
    if is_stack_signal_line(line) {
        return "stack trace".to_string();
    }
    if let Some(test_name) = generic_failed_test_name(line) {
        return format!("failed test: {test_name}");
    }
    if is_exception_signal_line(line) {
        return format!("exception: {}", truncate_command_line(line.trim(), 96));
    }
    format!("diagnostic: {}", truncate_command_line(line.trim(), 96))
}

fn push_labeled_lines(output: &mut Vec<String>, label: &str, lines: &[String], limit: usize) {
    if lines.is_empty() {
        return;
    }

    output.push(format!("{label} ({}):", lines.len()));
    for line in lines.iter().take(limit.max(1)) {
        output.push(format!("  {line}"));
    }
    if lines.len() > limit.max(1) {
        output.push(format!(
            "  [... {} more {label} ...]",
            lines.len() - limit.max(1)
        ));
    }
}

fn push_head_tail_lines(
    output: &mut Vec<String>,
    lines: &[String],
    limit: usize,
    max_line_chars: usize,
    omitted_label: &str,
    prefix: &str,
) {
    let limit = limit.max(1);
    if lines.len() <= limit {
        for line in lines {
            output.push(format!(
                "{prefix}{}",
                truncate_command_line(line, max_line_chars)
            ));
        }
        return;
    }

    let head = limit.div_ceil(2);
    let tail = limit.saturating_sub(head);
    for line in lines.iter().take(head) {
        output.push(format!(
            "{prefix}{}",
            truncate_command_line(line, max_line_chars)
        ));
    }
    output.push(format!(
        "{prefix}[... omitted {} {omitted_label} ...]",
        lines.len().saturating_sub(head + tail)
    ));
    for line in lines.iter().skip(lines.len().saturating_sub(tail)) {
        output.push(format!(
            "{prefix}{}",
            truncate_command_line(line, max_line_chars)
        ));
    }
}

fn push_unique_line(lines: &mut Vec<String>, line: &str) {
    if !line.is_empty() && !lines.iter().any(|existing| existing == line) {
        lines.push(line.to_string());
    }
}

fn push_unique_truncated_line(lines: &mut Vec<String>, line: &str, max_chars: usize) {
    let line = truncate_command_line(line.trim(), max_chars);
    push_unique_line(lines, &line);
}

fn smart_truncate_command_output(input: &str, options: &CommandOutputCompactOptions) -> String {
    let lines = command_lines(input);
    if lines.is_empty() {
        return String::new();
    }

    let max_lines = options.max_lines.max(1);
    let mut output = Vec::new();
    if lines.len() <= max_lines {
        for line in lines {
            output.push(truncate_command_line(line, options.max_line_chars));
        }
        return lines_to_text(output);
    }

    if let Some(output) = smart_truncate_with_critical_lines(&lines, options, max_lines) {
        return output;
    }

    let (head, tail) = bounded_head_tail(options, max_lines);
    for line in lines.iter().take(head) {
        output.push(truncate_command_line(line, options.max_line_chars));
    }
    output.push(format!(
        "[... omitted {} lines ...]",
        lines.len().saturating_sub(head + tail)
    ));
    for line in lines.iter().skip(lines.len().saturating_sub(tail)) {
        output.push(truncate_command_line(line, options.max_line_chars));
    }
    lines_to_text(output)
}

fn smart_truncate_with_critical_lines(
    lines: &[&str],
    options: &CommandOutputCompactOptions,
    max_lines: usize,
) -> Option<String> {
    let critical = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| is_critical_preserve_line(line))
        .map(|(index, line)| (index, *line))
        .collect::<Vec<_>>();
    if critical.is_empty() {
        return None;
    }

    if max_lines <= 12 {
        let mut output = Vec::new();
        output.push(format!(
            "sum: output lines={}, critical={}",
            lines.len(),
            critical.len()
        ));
        output.push("critical lines:".to_string());
        push_numbered_critical_lines(
            &mut output,
            &critical,
            max_lines.saturating_sub(2).max(1),
            options,
        );
        return Some(lines_to_text(output));
    }

    let section_overhead = 4usize;
    let context_budget = max_lines.saturating_sub(section_overhead).max(1);
    let mut head = options
        .head_lines
        .min(context_budget.saturating_div(4).max(1));
    let mut tail = options
        .tail_lines
        .min(context_budget.saturating_div(4).max(1));
    while head.saturating_add(tail) >= context_budget && tail > 0 {
        tail -= 1;
    }
    while head.saturating_add(tail) >= context_budget && head > 0 {
        head -= 1;
    }
    let critical_budget = context_budget.saturating_sub(head + tail).max(1);

    let mut output = Vec::new();
    output.push(format!(
        "sum: output lines={}, critical={}",
        lines.len(),
        critical.len()
    ));
    output.push("head:".to_string());
    for line in lines.iter().take(head) {
        output.push(truncate_command_line(line, options.max_line_chars));
    }
    output.push("critical lines:".to_string());

    let filtered_critical = critical
        .iter()
        .filter(|(index, _)| *index >= head && *index < lines.len().saturating_sub(tail))
        .copied()
        .collect::<Vec<_>>();
    push_numbered_critical_lines(&mut output, &filtered_critical, critical_budget, options);

    output.push("tail:".to_string());
    for line in lines.iter().skip(lines.len().saturating_sub(tail)) {
        output.push(truncate_command_line(line, options.max_line_chars));
    }

    Some(lines_to_text(output))
}

fn push_numbered_critical_lines(
    output: &mut Vec<String>,
    critical: &[(usize, &str)],
    budget: usize,
    options: &CommandOutputCompactOptions,
) {
    if critical.is_empty() || budget == 0 {
        return;
    }

    if critical.len() <= budget {
        for (index, line) in critical {
            output.push(format!(
                "  L{}: {}",
                index + 1,
                truncate_command_line(line.trim(), options.max_line_chars)
            ));
        }
        return;
    }

    if budget == 1 {
        if let Some((index, line)) = critical.first() {
            output.push(format!(
                "  L{}: {}",
                index + 1,
                truncate_command_line(line.trim(), options.max_line_chars)
            ));
        }
        return;
    }

    let mut candidates = critical.to_vec();
    candidates.sort_by_key(|(index, line)| (critical_preserve_priority(line), *index));
    let selected_budget = budget.saturating_sub(1).max(1);
    let mut selected = Vec::new();
    let mut seen = BTreeMap::<String, ()>::new();
    let failure_present = critical
        .iter()
        .any(|(_, line)| is_failure_first_critical_line(line));
    for candidate in candidates {
        if failure_present && is_warning_only_signal_line(candidate.1) {
            continue;
        }
        let key = critical_line_selection_key(candidate.1);
        if seen.insert(key, ()).is_some() {
            continue;
        }
        selected.push(candidate);
        if selected.len() >= selected_budget {
            break;
        }
    }
    selected.sort_by_key(|(index, _)| *index);

    for (index, line) in &selected {
        output.push(format!(
            "  L{}: {}",
            index + 1,
            truncate_command_line(line.trim(), options.max_line_chars)
        ));
    }
    output.push(format!(
        "  [... omitted {} critical lines ...]",
        critical.len().saturating_sub(selected.len())
    ));
}

fn critical_line_selection_key(line: &str) -> String {
    line.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn critical_preserve_priority(line: &str) -> u8 {
    if is_generated_compaction_header_line(line) {
        return 5;
    }
    if is_warning_only_signal_line(line) {
        return 4;
    }
    if is_error_signal_line(line)
        || is_test_failure_signal_line(line)
        || is_diagnostic_failure_summary_line(line)
        || is_rust_exit_status_line(line)
    {
        return 0;
    }
    if count_file_location_signals(line) > 0 || is_stack_signal_line(line) {
        return 1;
    }
    if is_rust_diagnostic_signal_line(line) || is_log_level_signal_line(line) {
        return 2;
    }
    3
}

fn is_failure_first_critical_line(line: &str) -> bool {
    !is_warning_only_signal_line(line)
        && (is_error_signal_line(line)
            || is_test_failure_signal_line(line)
            || is_diagnostic_failure_summary_line(line)
            || is_rust_exit_status_line(line))
}

pub(crate) fn is_generated_compaction_header_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    lower.starts_with("pcs:")
        || lower.starts_with("# prodex context saver:")
        || lower.starts_with("sum:")
        || lower.starts_with("rust/cargo summary:")
        || lower.starts_with("diagnostic summary:")
        || lower.starts_with("success output summary:")
        || lower.starts_with("command output summary:")
        || lower.starts_with("baseline compaction:")
        || lower.starts_with("base:")
        || lower.starts_with("intent matches:")
        || lower.starts_with("int:")
        || lower.starts_with("diagnostics (")
        || lower.starts_with("locations (")
        || lower.starts_with("failed tests (")
        || lower.starts_with("exit statuses (")
        || lower.starts_with("key lines (")
        || lower.starts_with("critical blocks:")
}

fn is_warning_only_signal_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    (lower.starts_with("warning")
        || lower.contains(" warning ")
        || lower.contains("warning ts")
        || lower.contains(": warning ts")
        || lower.contains(" - warning ts"))
        && !lower.contains("error")
        && !lower.contains("failed")
        && !lower.contains("panicked")
}

fn finalize_compacted_command_output(
    kind: CommandOutputKind,
    original: &str,
    mut lines: Vec<String>,
    options: &CommandOutputCompactOptions,
) -> String {
    let original_lines = count_text_lines(original);
    let body_lines = lines.len();
    let mut output = Vec::new();
    output.push(format!(
        "pcs: {} ({}->{})",
        kind.label(),
        original_lines,
        body_lines,
    ));
    output.append(&mut lines);
    let text = lines_to_text(output);
    if count_text_lines(&text) > options.max_lines.saturating_add(1).max(2) {
        smart_truncate_command_output(&text, options)
    } else {
        text
    }
}

fn canonicalize_compacted_command_paths(
    original: &str,
    output: &str,
    kind: CommandOutputKind,
) -> String {
    if !command_output_kind_allows_repo_relative_paths(kind) {
        return output.to_string();
    }

    let aliases = repeated_repo_path_aliases(original, output);
    if aliases.is_empty() {
        return output.to_string();
    }

    if let Some(rewritten) = rewrite_absolute_paths_with_aliases(output, &aliases)
        && critical_signal_self_check(output, &rewritten).passed()
    {
        return rewritten;
    }

    // Compatibility fallback: older compaction removed repeated repo prefixes entirely.
    let mut relative = output.to_string();
    for alias in aliases {
        relative = replace_absolute_path_prefix(&relative, &alias.prefix);
    }
    if relative != output && critical_signal_self_check(output, &relative).passed() {
        relative
    } else {
        output.to_string()
    }
}

fn command_output_kind_allows_repo_relative_paths(kind: CommandOutputKind) -> bool {
    matches!(
        kind,
        CommandOutputKind::GitStatus
            | CommandOutputKind::GitDiff
            | CommandOutputKind::RustDiagnostics
            | CommandOutputKind::Diagnostics
            | CommandOutputKind::GitLog
            | CommandOutputKind::Search
            | CommandOutputKind::FileList
            | CommandOutputKind::NoisySuccess
            | CommandOutputKind::Plain
    )
}

fn repeated_repo_path_aliases(original: &str, output: &str) -> Vec<CommandPathAlias> {
    let mut prefixes = repeated_repo_relative_path_prefixes(original);
    for prefix in repeated_repo_relative_path_prefixes(output) {
        push_unique_line(&mut prefixes, &prefix);
    }
    prefixes.retain(|prefix| count_absolute_path_prefix_occurrences(output, prefix) >= 2);
    prefixes.sort_by_key(|prefix| Reverse(prefix.len()));

    let mut selected = Vec::<String>::new();
    for prefix in prefixes {
        if selected
            .iter()
            .any(|existing| path_prefix_contains(existing, &prefix))
        {
            continue;
        }
        selected.push(prefix);
    }

    selected
        .into_iter()
        .enumerate()
        .map(|(index, prefix)| CommandPathAlias {
            alias: if index == 0 {
                "$REPO".to_string()
            } else {
                format!("$PATH{index}")
            },
            prefix,
        })
        .collect()
}

fn rewrite_absolute_paths_with_aliases(
    output: &str,
    aliases: &[CommandPathAlias],
) -> Option<String> {
    let mut rewritten = output.to_string();
    for alias in aliases {
        rewritten =
            replace_absolute_path_prefix_with_alias(&rewritten, &alias.prefix, &alias.alias);
    }
    if rewritten == output {
        return None;
    }

    let mapping = aliases
        .iter()
        .map(|alias| format!("{}={}", alias.alias, alias.prefix))
        .collect::<Vec<_>>()
        .join(", ");
    Some(insert_path_alias_mapping_line(
        &rewritten,
        &format!("path aliases: {mapping}"),
    ))
}

fn insert_path_alias_mapping_line(output: &str, mapping_line: &str) -> String {
    let mut lines = command_lines(output)
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let insert_at = usize::from(
        lines
            .first()
            .is_some_and(|line| is_generated_compaction_header_line(line)),
    );
    lines.insert(insert_at, mapping_line.to_string());
    lines_to_text(lines)
}

fn repeated_repo_relative_path_prefixes(input: &str) -> Vec<String> {
    let cwd_prefixes = repeated_current_directory_prefixes(input);
    if !cwd_prefixes.is_empty() {
        return cwd_prefixes;
    }
    repeated_unambiguous_marker_path_prefixes(input)
}

fn repeated_current_directory_prefixes(input: &str) -> Vec<String> {
    let mut prefixes = Vec::<String>::new();
    if let Ok(cwd) = std::env::current_dir()
        && let Some(prefix) = normalize_absolute_path_prefix(&cwd)
    {
        push_unique_line(&mut prefixes, &prefix);
    }
    if let Some(pwd) = std::env::var_os("PWD").map(PathBuf::from)
        && let Some(prefix) = normalize_absolute_path_prefix(&pwd)
    {
        push_unique_line(&mut prefixes, &prefix);
    }

    prefixes.retain(|prefix| count_absolute_path_prefix_occurrences(input, prefix) >= 2);
    prefixes.sort_by_key(|prefix| Reverse(prefix.len()));

    let mut selected = Vec::<String>::new();
    for prefix in prefixes {
        if selected
            .iter()
            .any(|existing| path_prefix_contains(existing, &prefix))
        {
            continue;
        }
        selected.push(prefix);
    }
    selected
}

fn repeated_unambiguous_marker_path_prefixes(text: &str) -> Vec<String> {
    const REPO_MARKERS: &[&str] = &[
        "/crates/",
        "/src/",
        "/tests/",
        "/benches/",
        "/examples/",
        "/README.md",
        "/Cargo.toml",
    ];

    let mut prefix_counts = BTreeMap::<String, usize>::new();
    for marker in REPO_MARKERS {
        let mut search_start = 0usize;
        while let Some(offset) = text[search_start..].find(marker) {
            let marker_start = search_start + offset;
            if let Some(prefix) = absolute_path_prefix_before_marker(text, marker_start) {
                *prefix_counts.entry(prefix).or_default() += 1;
            }
            search_start = marker_start.saturating_add(marker.len());
            if search_start >= text.len() {
                break;
            }
        }
    }

    let prefixes = prefix_counts
        .into_iter()
        .filter(|(prefix, count)| {
            *count >= 2 && prefix.len() > 1 && canonicalizable_absolute_path_prefix(prefix)
        })
        .map(|(prefix, _)| prefix)
        .collect::<Vec<_>>();
    if prefixes.len() != 1 {
        return Vec::new();
    }
    prefixes
}

fn normalize_absolute_path_prefix(path: &Path) -> Option<String> {
    if !path.is_absolute() {
        return None;
    }
    let prefix = path.display().to_string().replace('\\', "/");
    let prefix = prefix.trim_end_matches('/').to_string();
    if prefix.is_empty() || prefix == "/" {
        return None;
    }
    Some(prefix)
}

fn path_prefix_contains(parent: &str, child: &str) -> bool {
    child == parent
        || child
            .strip_prefix(parent)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn count_absolute_path_prefix_occurrences(text: &str, prefix: &str) -> usize {
    absolute_path_prefix_occurrences(text, prefix).len()
}

fn replace_absolute_path_prefix(text: &str, prefix: &str) -> String {
    let occurrences = absolute_path_prefix_occurrences(text, prefix);
    if occurrences.is_empty() {
        return text.to_string();
    }

    let marker_len = prefix.len() + 1;
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for start in occurrences {
        output.push_str(&text[cursor..start]);
        cursor = start + marker_len;
    }
    output.push_str(&text[cursor..]);
    output
}

fn replace_absolute_path_prefix_with_alias(text: &str, prefix: &str, alias: &str) -> String {
    let occurrences = absolute_path_prefix_occurrences(text, prefix);
    if occurrences.is_empty() {
        return text.to_string();
    }

    let marker_len = prefix.len() + 1;
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for start in occurrences {
        output.push_str(&text[cursor..start]);
        output.push_str(alias);
        output.push('/');
        cursor = start + marker_len;
    }
    output.push_str(&text[cursor..]);
    output
}

fn absolute_path_prefix_occurrences(text: &str, prefix: &str) -> Vec<usize> {
    let marker = format!("{prefix}/");
    let mut occurrences = Vec::new();
    let mut search_start = 0usize;
    while let Some(relative_start) = text[search_start..].find(&marker) {
        let start = search_start + relative_start;
        let suffix_start = start + marker.len();
        if is_absolute_path_prefix_boundary(text, start)
            && is_repo_relative_suffix_candidate(&text[suffix_start..])
        {
            occurrences.push(start);
        }
        search_start = suffix_start;
    }
    occurrences
}

fn is_absolute_path_prefix_boundary(text: &str, start: usize) -> bool {
    let Some(previous) = text[..start].chars().next_back() else {
        return true;
    };
    if previous != '/' && previous != '\\' {
        return !previous.is_ascii_alphanumeric();
    }

    let before = &text[..start];
    before.ends_with("a/") || before.ends_with("b/")
}

fn is_repo_relative_suffix_candidate(suffix: &str) -> bool {
    let first_segment = suffix
        .split(|ch: char| {
            ch == '/'
                || ch == '\\'
                || ch == ':'
                || ch == '"'
                || ch == '\''
                || ch == '`'
                || ch == ')'
                || ch == ']'
                || ch == '}'
                || ch.is_whitespace()
        })
        .next()
        .unwrap_or_default();
    !matches!(first_segment, "" | "." | "..")
}

fn absolute_path_prefix_before_marker(text: &str, marker_start: usize) -> Option<String> {
    let before = text.get(..marker_start)?;
    let path_start = before
        .char_indices()
        .rev()
        .find(|(_, ch)| !is_absolute_path_prefix_char(*ch))
        .map(|(index, ch)| index + ch.len_utf8())
        .unwrap_or(0);
    let prefix = text.get(path_start..marker_start)?;
    (prefix.starts_with('/') && prefix.len() > 1).then(|| prefix.to_string())
}

fn is_absolute_path_prefix_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-')
}

fn canonicalizable_absolute_path_prefix(prefix: &str) -> bool {
    !prefix.contains("/rustc/")
        && !prefix.contains("/.rustup/")
        && !prefix.contains("/.cargo/registry/")
        && !prefix.contains("/.cargo/git/")
}

fn detect_command_output_kind(input: &str) -> CommandOutputKind {
    let lines = command_lines(input);
    if looks_like_git_log_stat_output(&lines) {
        return CommandOutputKind::GitLog;
    }

    if looks_like_git_diff_output(&lines) {
        return CommandOutputKind::GitDiff;
    }

    if looks_like_rust_diagnostic_output(&lines) {
        return CommandOutputKind::RustDiagnostics;
    }

    if looks_like_noisy_success_output(&lines) {
        return CommandOutputKind::NoisySuccess;
    }

    if looks_like_log_stream_output(&lines) {
        return CommandOutputKind::LogStream;
    }

    if looks_like_diagnostic_output(&lines) {
        return CommandOutputKind::Diagnostics;
    }

    if lines.iter().any(|line| {
        line.starts_with("On branch ")
            || line.starts_with("HEAD detached ")
            || line.starts_with("Changes to be committed:")
            || line.starts_with("Changes not staged for commit:")
            || line.starts_with("Untracked files:")
    }) || lines
        .iter()
        .filter(|line| is_short_git_status_line(line) || line.starts_with("## "))
        .take(3)
        .count()
        >= 2
    {
        return CommandOutputKind::GitStatus;
    }

    let non_empty = lines.iter().filter(|line| !line.trim().is_empty()).count();
    let search_matches = lines
        .iter()
        .filter(|line| {
            parse_search_match_line(line).is_some() || parse_rg_json_match_line(line).is_some()
        })
        .count();
    let heading_search_matches = count_heading_search_matches(&lines);
    let rg_json_lines = lines
        .iter()
        .filter(|line| looks_like_rg_json_line(line))
        .count();
    let total_search_matches = search_matches.saturating_add(heading_search_matches);
    if total_search_matches >= 2 && total_search_matches.saturating_mul(2) >= non_empty {
        return CommandOutputKind::Search;
    }
    if search_matches > 0 && rg_json_lines.saturating_mul(2) >= non_empty {
        return CommandOutputKind::Search;
    }

    let file_list_lines = lines
        .iter()
        .filter(|line| parse_file_list_entry_line(line).is_some())
        .count();
    if file_list_lines >= 4 && file_list_lines.saturating_mul(2) >= non_empty {
        return CommandOutputKind::FileList;
    }

    CommandOutputKind::Plain
}

fn detect_command_output_kind_with_hint(
    input: &str,
    kind_hint: Option<CommandOutputKind>,
) -> CommandOutputKind {
    let detected = detect_command_output_kind(input);
    if detected == CommandOutputKind::Plain {
        kind_hint
            .filter(|kind| *kind != CommandOutputKind::Auto)
            .unwrap_or(detected)
    } else {
        detected
    }
}

pub fn infer_command_output_kind_from_metadata(metadata: &str) -> Option<CommandOutputKind> {
    let tokens = command_metadata_tokens(metadata);
    infer_command_output_kind_from_metadata_tokens(&tokens)
}

fn infer_command_output_kind_from_metadata_tokens(tokens: &[String]) -> Option<CommandOutputKind> {
    for index in 0..tokens.len() {
        let command = command_metadata_token_command_name(&tokens[index]);
        if matches!(command, "rg" | "ripgrep" | "grep" | "egrep" | "fgrep") {
            return Some(CommandOutputKind::Search);
        }
        if matches!(command, "ls" | "find" | "tree") {
            return Some(CommandOutputKind::FileList);
        }
        if matches!(
            command,
            "pytest"
                | "py.test"
                | "tsc"
                | "ruff"
                | "mypy"
                | "biome"
                | "oxlint"
                | "eslint"
                | "playwright"
                | "cypress"
        ) || command.ends_with("-tsc")
            || command.ends_with("_tsc")
        {
            return Some(CommandOutputKind::Diagnostics);
        }
        if matches!(
            command,
            "bazel"
                | "bazelisk"
                | "nx"
                | "turbo"
                | "pip"
                | "pip3"
                | "uv"
                | "nyc"
                | "c8"
                | "vite"
                | "next"
        ) {
            return Some(CommandOutputKind::NoisySuccess);
        }
        if matches!(command, "gradle" | "gradlew")
            && command_metadata_subcommand_after(tokens, index)
                .is_some_and(|subcommand| matches!(subcommand, "test" | "check" | "build"))
        {
            return Some(CommandOutputKind::NoisySuccess);
        }
        if matches!(command, "mvn" | "mvnw")
            && command_metadata_subcommand_after(tokens, index).is_some_and(|subcommand| {
                matches!(subcommand, "test" | "verify" | "package" | "install")
            })
        {
            return Some(CommandOutputKind::NoisySuccess);
        }
        if matches!(command, "journalctl" | "tail")
            || command == "kubectl"
                && command_metadata_subcommand_after(tokens, index) == Some("logs")
        {
            return Some(CommandOutputKind::LogStream);
        }
        if command == "go"
            && command_metadata_subcommand_after(tokens, index)
                .is_some_and(|subcommand| matches!(subcommand, "vet" | "test" | "build"))
        {
            return Some(CommandOutputKind::Diagnostics);
        }
        if command == "cargo"
            && command_metadata_subcommand_after(tokens, index).is_some_and(|subcommand| {
                matches!(
                    subcommand,
                    "test" | "check" | "clippy" | "build" | "doc" | "nextest" | "fmt" | "fix"
                )
            })
        {
            return Some(CommandOutputKind::RustDiagnostics);
        }
        if command == "cargo"
            && command_metadata_subcommand_after(tokens, index)
                .is_some_and(|subcommand| matches!(subcommand, "update" | "install" | "fetch"))
        {
            return Some(CommandOutputKind::NoisySuccess);
        }
        if command == "git"
            && let Some(subcommand) = command_metadata_subcommand_after(tokens, index)
        {
            match subcommand {
                "status" => return Some(CommandOutputKind::GitStatus),
                "diff" | "show" => return Some(CommandOutputKind::GitDiff),
                "log" => return Some(CommandOutputKind::GitLog),
                "grep" => return Some(CommandOutputKind::Search),
                "ls-files" => return Some(CommandOutputKind::FileList),
                _ => {}
            }
        }
        if command == "docker"
            && command_metadata_subcommand_after(tokens, index) == Some("compose")
        {
            return Some(CommandOutputKind::NoisySuccess);
        }
        if command == "docker"
            && command_metadata_subcommand_after(tokens, index)
                .is_some_and(|subcommand| matches!(subcommand, "build" | "buildx" | "pull"))
        {
            return Some(CommandOutputKind::NoisySuccess);
        }
        if matches!(command, "npm" | "pnpm" | "yarn" | "bun")
            && command_metadata_package_script_after(tokens, index).is_some()
        {
            return Some(CommandOutputKind::Diagnostics);
        }
        if matches!(command, "npm" | "pnpm" | "yarn" | "bun")
            && command_metadata_package_install_after(tokens, index).is_some()
        {
            return Some(CommandOutputKind::NoisySuccess);
        }
        if matches!(command, "docker-compose") {
            return Some(CommandOutputKind::NoisySuccess);
        }
    }
    None
}

fn command_metadata_subcommand_after(tokens: &[String], command_index: usize) -> Option<&str> {
    let mut skip_next = false;
    for token in tokens
        .iter()
        .skip(command_index + 1)
        .map(|token| command_metadata_token_command_name(token))
    {
        if skip_next {
            skip_next = false;
            continue;
        }
        if command_metadata_token_option_takes_value(token) {
            skip_next = true;
            continue;
        }
        if !command_metadata_token_is_option_or_shell_glue(token) {
            return Some(token);
        }
    }
    None
}

fn command_metadata_package_script_after(tokens: &[String], command_index: usize) -> Option<&str> {
    let mut saw_run = false;
    let mut skip_next = false;
    for token in tokens
        .iter()
        .skip(command_index + 1)
        .map(|token| command_metadata_token_command_name(token))
    {
        if skip_next {
            skip_next = false;
            continue;
        }
        if command_metadata_token_option_takes_value(token) {
            skip_next = true;
            continue;
        }
        if command_metadata_token_is_option_or_shell_glue(token) {
            continue;
        }
        if token == "run" || token == "run-script" {
            saw_run = true;
            continue;
        }
        if matches!(
            token,
            "test" | "t" | "typecheck" | "type-check" | "tsc" | "check"
        ) || (saw_run && (token.contains("test") || token.contains("typecheck")))
        {
            return Some(token);
        }
        return None;
    }
    None
}

fn command_metadata_package_install_after(tokens: &[String], command_index: usize) -> Option<&str> {
    let mut skip_next = false;
    for token in tokens
        .iter()
        .skip(command_index + 1)
        .map(|token| command_metadata_token_command_name(token))
    {
        if skip_next {
            skip_next = false;
            continue;
        }
        if command_metadata_token_option_takes_value(token) {
            skip_next = true;
            continue;
        }
        if command_metadata_token_is_option_or_shell_glue(token) {
            continue;
        }
        if matches!(
            token,
            "install" | "i" | "ci" | "add" | "update" | "upgrade" | "sync"
        ) {
            return Some(token);
        }
        return None;
    }
    None
}

fn command_metadata_token_is_option_or_shell_glue(token: &str) -> bool {
    token.is_empty()
        || token.starts_with('-')
        || token.starts_with('+')
        || matches!(
            token,
            "cmd"
                | "command"
                | "args"
                | "arguments"
                | "metadata"
                | "name"
                | "tool"
                | "tool_name"
                | "shell"
                | "bash"
                | "sh"
                | "zsh"
                | "fish"
                | "powershell"
                | "pwsh"
                | "python"
                | "python3"
                | "py"
                | "node"
                | "npx"
                | "bunx"
                | "uv"
                | "uvx"
                | "poetry"
                | "pipenv"
                | "exec_command"
                | "function_call"
                | "function_call_output"
                | "shell_call"
                | "shell_call_output"
                | "true"
                | "false"
                | "null"
        )
}

fn command_metadata_token_option_takes_value(token: &str) -> bool {
    matches!(
        token,
        "-c" | "-m"
            | "-p"
            | "--config"
            | "--git-dir"
            | "--work-tree"
            | "--manifest-path"
            | "--package"
            | "--bin"
            | "--example"
            | "--target"
            | "--project"
            | "--cwd"
            | "--prefix"
            | "--directory"
    )
}

fn command_metadata_token_command_name(token: &str) -> &str {
    let basename = token.rsplit('/').next().unwrap_or(token);
    basename.strip_suffix(".exe").unwrap_or(basename)
}

fn command_metadata_tokens(metadata: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    for ch in metadata.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | '+') {
            token.push(ch.to_ascii_lowercase());
        } else if !token.is_empty() {
            tokens.push(std::mem::take(&mut token));
        }
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    tokens
}

pub(crate) fn normalize_command_output(input: &str) -> String {
    let stripped = strip_ansi_codes(input);
    let mut lines = stripped
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect::<Vec<_>>();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines_to_text(lines)
}

fn strip_ansi_codes(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            match chars.peek().copied() {
                Some('[') => {
                    chars.next();
                    for code in chars.by_ref() {
                        if ('@'..='~').contains(&code) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    let mut previous = '\0';
                    for code in chars.by_ref() {
                        if code == '\u{7}' || (previous == '\u{1b}' && code == '\\') {
                            break;
                        }
                        previous = code;
                    }
                }
                Some(_) => {
                    chars.next();
                }
                None => {}
            }
        } else if ch == '\r' {
            if !matches!(chars.peek(), Some('\n')) {
                output.push('\n');
            }
        } else {
            output.push(ch);
        }
    }
    output
}

pub(crate) fn command_lines(input: &str) -> Vec<&str> {
    input
        .trim_end_matches('\n')
        .split('\n')
        .filter(|line| !(line.is_empty() && input.is_empty()))
        .collect()
}

pub(crate) fn count_text_lines(input: &str) -> usize {
    if input.is_empty() {
        0
    } else {
        input.trim_end_matches('\n').split('\n').count()
    }
}

fn lines_to_text(lines: Vec<String>) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

fn truncate_command_line(line: &str, max_chars: usize) -> String {
    let max_chars = max_chars.max(24);
    let char_count = line.chars().count();
    if char_count <= max_chars {
        return line.to_string();
    }

    let tail_chars = 16.min(max_chars / 3);
    let head_chars = max_chars.saturating_sub(tail_chars).saturating_sub(24);
    let head = line.chars().take(head_chars).collect::<String>();
    let tail = line
        .chars()
        .rev()
        .take(tail_chars)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!(
        "{head} [... {} chars omitted ...] {tail}",
        char_count.saturating_sub(head_chars + tail_chars),
    )
}

fn bounded_head_tail(options: &CommandOutputCompactOptions, max_lines: usize) -> (usize, usize) {
    let requested_head = options.head_lines.min(max_lines);
    let requested_tail = options
        .tail_lines
        .min(max_lines.saturating_sub(requested_head));
    if requested_head + requested_tail == 0 {
        (max_lines, 0)
    } else {
        (requested_head, requested_tail)
    }
}

#[derive(Default)]
struct GitStatusSummary {
    branch: Option<String>,
    staged: Vec<String>,
    modified: Vec<String>,
    deleted: Vec<String>,
    renamed: Vec<String>,
    conflicted: Vec<String>,
    untracked: Vec<String>,
    other: Vec<String>,
    clean: bool,
}

fn is_short_git_status_line(line: &str) -> bool {
    if line.len() < 3 {
        return false;
    }
    let bytes = line.as_bytes();
    let valid_status = |byte: u8| {
        matches!(
            byte,
            b' ' | b'M' | b'A' | b'D' | b'R' | b'C' | b'U' | b'?' | b'!'
        )
    };
    valid_status(bytes[0]) && valid_status(bytes[1]) && bytes[2] == b' '
}

fn parse_short_git_status_line(line: &str, summary: &mut GitStatusSummary) {
    if let Some(branch) = line.strip_prefix("## ") {
        summary.branch = Some(branch.trim().to_string());
        return;
    }
    if !is_short_git_status_line(line) {
        if !line.trim().is_empty() {
            summary.other.push(line.trim().to_string());
        }
        return;
    }

    let status = &line[..2];
    let path = line[3..].trim();
    if status == "??" {
        summary.untracked.push(path.to_string());
        return;
    }
    if status.contains('U') {
        summary
            .conflicted
            .push(format!("{} {}", status.trim(), path));
        return;
    }

    let mut chars = status.chars();
    let index = chars.next().unwrap_or(' ');
    let worktree = chars.next().unwrap_or(' ');
    push_short_status_path(index, path, true, summary);
    push_short_status_path(worktree, path, false, summary);
}

fn push_short_status_path(status: char, path: &str, index: bool, summary: &mut GitStatusSummary) {
    match status {
        'M' | 'A' => {
            if index {
                summary.staged.push(format!("{status} {path}"));
            } else {
                summary.modified.push(format!("{status} {path}"));
            }
        }
        'D' => summary.deleted.push(format!("{status} {path}")),
        'R' | 'C' => summary.renamed.push(format!("{status} {path}")),
        '?' => summary.untracked.push(path.to_string()),
        ' ' | '!' => {}
        other => summary.other.push(format!("{other} {path}")),
    }
}

fn parse_long_git_status_lines(lines: &[&str], summary: &mut GitStatusSummary) {
    let mut section = GitStatusSection::Other;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("(use ") {
            continue;
        }
        if let Some(branch) = trimmed.strip_prefix("On branch ") {
            summary.branch = Some(branch.trim().to_string());
            continue;
        }
        if trimmed.starts_with("HEAD detached ") {
            summary.branch = Some(trimmed.to_string());
            continue;
        }
        if trimmed.contains("nothing to commit") || trimmed.contains("working tree clean") {
            summary.clean = true;
            continue;
        }
        section = match trimmed {
            "Changes to be committed:" => GitStatusSection::Staged,
            "Changes not staged for commit:" => GitStatusSection::Modified,
            "Untracked files:" => GitStatusSection::Untracked,
            "Unmerged paths:" => GitStatusSection::Conflicted,
            _ => section,
        };
        if trimmed.ends_with(':') {
            continue;
        }

        match section {
            GitStatusSection::Staged => summary.staged.push(parse_long_status_path(trimmed)),
            GitStatusSection::Modified => {
                let parsed = parse_long_status_path(trimmed);
                if parsed.starts_with("deleted:") {
                    summary.deleted.push(parsed);
                } else {
                    summary.modified.push(parsed);
                }
            }
            GitStatusSection::Untracked => summary.untracked.push(trimmed.to_string()),
            GitStatusSection::Conflicted => {
                summary.conflicted.push(parse_long_status_path(trimmed))
            }
            GitStatusSection::Other => {
                if !trimmed.starts_with("Your branch ") {
                    summary.other.push(trimmed.to_string());
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum GitStatusSection {
    Staged,
    Modified,
    Untracked,
    Conflicted,
    Other,
}

fn parse_long_status_path(trimmed: &str) -> String {
    trimmed
        .split_once(':')
        .map(|(status, path)| format!("{}: {}", status.trim(), path.trim()))
        .unwrap_or_else(|| trimmed.to_string())
}

fn push_item_summary(output: &mut Vec<String>, label: &str, items: &[String], limit: usize) {
    if items.is_empty() {
        return;
    }

    let mut unique = Vec::<&str>::new();
    for item in items {
        if !unique.iter().any(|existing| *existing == item) {
            unique.push(item);
        }
    }

    let mut rendered = unique
        .iter()
        .take(limit)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if unique.len() > limit {
        rendered.push_str(&format!(" (+{} more)", unique.len() - limit));
    }
    output.push(format!("{label} ({}): {rendered}", unique.len()));
}

struct GitDiffSummary {
    path: String,
    added: usize,
    removed: usize,
    hunks: usize,
    binary: bool,
    semantic_contexts: Vec<String>,
}

#[derive(Default)]
struct RustDiagnosticSummary {
    errors: usize,
    warnings: usize,
    panics: usize,
    root_causes: Vec<String>,
    diagnostic_headers: Vec<String>,
    locations: Vec<String>,
    failed_tests: Vec<String>,
    exit_statuses: Vec<String>,
}

impl RustDiagnosticSummary {
    fn is_empty(&self) -> bool {
        self.errors == 0
            && self.warnings == 0
            && self.panics == 0
            && self.root_causes.is_empty()
            && self.diagnostic_headers.is_empty()
            && self.locations.is_empty()
            && self.failed_tests.is_empty()
            && self.exit_statuses.is_empty()
    }

    fn record_diagnostic(&mut self, severity: RustDiagnosticSeverity, line: &str) {
        match severity {
            RustDiagnosticSeverity::Error => self.errors += 1,
            RustDiagnosticSeverity::Warning => self.warnings += 1,
        }
        push_unique_line(&mut self.diagnostic_headers, line.trim());
        if matches!(severity, RustDiagnosticSeverity::Error) {
            push_unique_truncated_line(&mut self.root_causes, line, 240);
        }
    }

    fn record_failed_test(&mut self, test_name: &str) {
        push_unique_line(&mut self.failed_tests, test_name.trim());
        push_unique_line(&mut self.root_causes, test_name.trim());
    }

    fn record_location(&mut self, line: &str) {
        push_unique_line(&mut self.locations, line.trim());
    }

    fn record_exit_status(&mut self, line: &str) {
        push_unique_line(&mut self.exit_statuses, line.trim());
        push_unique_truncated_line(&mut self.root_causes, line, 240);
    }

    fn record_block_signals(&mut self, block: &RustCriticalBlock) {
        for line in &block.lines {
            if is_rust_location_line(line) {
                self.record_location(line);
            }
            if is_rust_panic_line(line) {
                self.panics += 1;
                push_unique_truncated_line(&mut self.root_causes, line, 240);
            }
            if is_rust_exit_status_line(line) {
                self.record_exit_status(line);
            }
            if let Some(test_name) = rust_failure_separator_name(line) {
                self.record_failed_test(test_name);
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum RustDiagnosticSeverity {
    Error,
    Warning,
}

struct RustCriticalBlock {
    label: String,
    lines: Vec<String>,
}

#[derive(Default)]
struct CommandDiagnosticSummary {
    errors: usize,
    stack_markers: usize,
    root_causes: Vec<String>,
    diagnostic_headers: Vec<String>,
    locations: Vec<String>,
    failed_tests: Vec<String>,
    exit_statuses: Vec<String>,
}

impl CommandDiagnosticSummary {
    fn is_empty(&self) -> bool {
        self.errors == 0
            && self.stack_markers == 0
            && self.root_causes.is_empty()
            && self.diagnostic_headers.is_empty()
            && self.locations.is_empty()
            && self.failed_tests.is_empty()
            && self.exit_statuses.is_empty()
    }

    fn record_line(&mut self, line: &str) {
        if is_error_signal_line(line) || is_typescript_diagnostic_line(line) {
            self.errors += 1;
            push_unique_truncated_line(&mut self.diagnostic_headers, line, 240);
            push_unique_truncated_line(&mut self.root_causes, line, 240);
        }
        if let Some(test_name) = generic_failed_test_name(line) {
            push_unique_line(&mut self.failed_tests, test_name);
            push_unique_line(&mut self.root_causes, test_name);
        }
        if count_file_location_signals(line) > 0 {
            push_unique_truncated_line(&mut self.locations, line, 240);
        }
        if is_rust_exit_status_line(line) {
            push_unique_truncated_line(&mut self.exit_statuses, line, 240);
            push_unique_truncated_line(&mut self.root_causes, line, 240);
        }
        if is_stack_signal_line(line) {
            self.stack_markers += 1;
            push_unique_truncated_line(&mut self.diagnostic_headers, line, 240);
        }
    }

    fn record_block_signals(&mut self, block: &CommandCriticalBlock) {
        for line in &block.lines {
            self.record_line(line);
        }
    }
}

struct CommandCriticalBlock {
    label: String,
    lines: Vec<String>,
}

#[derive(Default)]
struct GitLogCommitSummary {
    header: String,
    metadata: Vec<String>,
    subject: Vec<String>,
    stat_lines: Vec<String>,
    stat_summaries: Vec<String>,
}

fn split_git_diff_sections<'a>(lines: &'a [&'a str]) -> Vec<Vec<&'a str>> {
    let mut sections = Vec::new();
    let mut current = Vec::new();
    for line in lines {
        if line.starts_with("diff --git ") && !current.is_empty() {
            sections.push(current);
            current = Vec::new();
        }
        if line.starts_with("diff --git ")
            || !current.is_empty()
            || line.starts_with("--- ")
            || line.starts_with("@@ ")
        {
            current.push(*line);
        }
    }
    if !current.is_empty() {
        sections.push(current);
    }
    sections
}

fn summarize_git_diff_section(section: &[&str]) -> GitDiffSummary {
    let mut summary = GitDiffSummary {
        path: git_diff_section_path(section),
        added: 0,
        removed: 0,
        hunks: 0,
        binary: false,
        semantic_contexts: Vec::new(),
    };
    for line in section {
        if line.starts_with("@@ ") {
            summary.hunks += 1;
            if let Some(context) = git_diff_semantic_context_line(line) {
                push_unique_truncated_line(&mut summary.semantic_contexts, &context, 120);
            }
        } else if line.starts_with('+') && !line.starts_with("+++") {
            summary.added += 1;
            if let Some(context) = git_diff_semantic_context_line(line) {
                push_unique_truncated_line(&mut summary.semantic_contexts, &context, 120);
            }
        } else if line.starts_with('-') && !line.starts_with("---") {
            summary.removed += 1;
            if let Some(context) = git_diff_semantic_context_line(line) {
                push_unique_truncated_line(&mut summary.semantic_contexts, &context, 120);
            }
        } else if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
            summary.binary = true;
        }
    }
    summary
}

fn git_diff_section_path(section: &[&str]) -> String {
    for line in section {
        if let Some((_, rhs)) = line.split_once(" b/") {
            return rhs.trim().to_string();
        }
    }
    for line in section {
        if let Some(path) = line.strip_prefix("+++ b/") {
            return path.trim().to_string();
        }
    }
    "unknown".to_string()
}

fn is_git_diff_structural_line(line: &str) -> bool {
    line.starts_with("diff --git ")
        || line.starts_with("index ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
        || line.starts_with("@@ ")
        || line.starts_with("new file mode ")
        || line.starts_with("deleted file mode ")
        || line.starts_with("old mode ")
        || line.starts_with("new mode ")
        || line.starts_with("rename from ")
        || line.starts_with("rename to ")
        || line.starts_with("similarity index ")
        || line.starts_with("dissimilarity index ")
        || line.starts_with("Binary files ")
        || line.starts_with("GIT binary patch")
}

fn is_git_diff_excerpt_structural_line(line: &str, intent_focused: bool) -> bool {
    if line.starts_with("@@ ")
        || line.starts_with("Binary files ")
        || line.starts_with("GIT binary patch")
    {
        return true;
    }
    if intent_focused {
        return false;
    }
    line.starts_with("diff --git ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
        || line.starts_with("new file mode ")
        || line.starts_with("deleted file mode ")
        || line.starts_with("old mode ")
        || line.starts_with("new mode ")
        || line.starts_with("rename from ")
        || line.starts_with("rename to ")
}

fn git_diff_semantic_context_line(line: &str) -> Option<String> {
    if line.starts_with("@@ ")
        && let Some((_, context)) = line.rsplit_once("@@")
    {
        let context = context.trim();
        if !context.is_empty() {
            return Some(context.to_string());
        }
    }

    let trimmed = line.trim_start_matches(['+', '-', ' ']).trim();
    if trimmed.is_empty()
        || trimmed.starts_with("//")
        || trimmed.starts_with('#') && !trimmed.starts_with("#[")
        || trimmed.starts_with('*')
    {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    let semantic_start = [
        "pub fn ",
        "async fn ",
        "fn ",
        "def ",
        "class ",
        "impl ",
        "pub struct ",
        "struct ",
        "pub enum ",
        "enum ",
        "interface ",
        "type ",
        "function ",
        "describe(",
        "it(",
        "test(",
        "#[test]",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));
    let semantic_contains = lower.contains(" test_")
        || lower.contains("::test_")
        || lower.contains(" should ")
        || lower.contains("=>")
            && (lower.contains("test") || lower.contains("describe") || lower.contains("it("));

    (semantic_start || semantic_contains).then(|| trimmed.to_string())
}

fn looks_like_git_diff_output(lines: &[&str]) -> bool {
    if lines.iter().any(|line| line.starts_with("diff --git "))
        || lines
            .iter()
            .filter(|line| is_diff_hunk_line(line))
            .take(2)
            .count()
            >= 1
    {
        return true;
    }

    let stat_lines = lines
        .iter()
        .filter(|line| looks_like_git_diff_stat_line(line))
        .count();
    let stat_summaries = lines
        .iter()
        .filter(|line| looks_like_git_diff_stat_summary(line))
        .count();
    stat_lines > 0 && stat_summaries > 0
}

fn looks_like_git_diff_stat_line(line: &str) -> bool {
    let trimmed = line.trim();
    let Some((path, stats)) = trimmed.split_once(" | ") else {
        return false;
    };
    if path.trim().is_empty() || stats.trim().is_empty() {
        return false;
    }
    let stats = stats.trim();
    stats.starts_with("Bin ")
        || stats.chars().any(|ch| ch == '+' || ch == '-')
        || stats
            .split_whitespace()
            .next()
            .is_some_and(|count| count.chars().all(|ch| ch.is_ascii_digit()))
}

fn looks_like_git_diff_stat_summary(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains(" file changed")
        || trimmed.contains(" files changed")
        || trimmed.contains(" insertion")
        || trimmed.contains(" deletion")
}

fn looks_like_git_log_stat_output(lines: &[&str]) -> bool {
    let commit_headers = lines
        .iter()
        .filter(|line| parse_git_log_commit_header(line).is_some())
        .count();
    if commit_headers == 0 {
        return false;
    }
    let stat_lines = lines
        .iter()
        .filter(|line| looks_like_git_diff_stat_line(line))
        .count();
    let stat_summaries = lines
        .iter()
        .filter(|line| looks_like_git_diff_stat_summary(line))
        .count();
    stat_lines > 0 || stat_summaries > 0
}

fn parse_git_log_stat_commits(lines: &[&str]) -> Vec<GitLogCommitSummary> {
    let mut commits = Vec::new();
    let mut current = None::<GitLogCommitSummary>;

    for line in lines {
        if let Some(header) = parse_git_log_commit_header(line) {
            if let Some(commit) = current.take()
                && (!commit.stat_lines.is_empty() || !commit.stat_summaries.is_empty())
            {
                commits.push(commit);
            }
            current = Some(GitLogCommitSummary {
                header,
                ..GitLogCommitSummary::default()
            });
            continue;
        }

        let Some(commit) = current.as_mut() else {
            continue;
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("Author:") || trimmed.starts_with("Date:") {
            push_unique_line(&mut commit.metadata, trimmed);
        } else if looks_like_git_diff_stat_line(line) {
            push_unique_line(&mut commit.stat_lines, trimmed);
        } else if looks_like_git_diff_stat_summary(line) {
            push_unique_line(&mut commit.stat_summaries, trimmed);
        } else if line.starts_with("    ") && commit.subject.len() < 2 {
            push_unique_line(&mut commit.subject, trimmed);
        }
    }

    if let Some(commit) = current.take()
        && (!commit.stat_lines.is_empty() || !commit.stat_summaries.is_empty())
    {
        commits.push(commit);
    }
    commits
}

fn parse_git_log_commit_header(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if let Some(hash) = trimmed.strip_prefix("commit ") {
        let hash = hash.split_whitespace().next().unwrap_or_default();
        if looks_like_git_hash(hash) {
            return Some(format!("commit {hash}"));
        }
    }

    let (hash, subject) = trimmed.split_once(' ')?;
    if looks_like_git_hash(hash) && !subject.trim().is_empty() {
        return Some(trimmed.to_string());
    }
    None
}

fn looks_like_git_hash(input: &str) -> bool {
    (7..=64).contains(&input.len()) && input.chars().all(|ch| ch.is_ascii_hexdigit())
}

#[derive(Clone)]
pub(crate) struct SearchMatch {
    pub(crate) path: String,
    pub(crate) line_number: Option<usize>,
    pub(crate) text: String,
}

pub(crate) fn parse_search_match_line(line: &str) -> Option<SearchMatch> {
    let (path, rest) = line.split_once(':')?;
    if path.trim().is_empty() || rest.trim().is_empty() {
        return None;
    }

    let (line_number, text) = if let Some((candidate, after_line)) = rest.split_once(':') {
        if candidate.chars().all(|ch| ch.is_ascii_digit()) {
            let text = if let Some((column, after_column)) = after_line.split_once(':') {
                if column.chars().all(|ch| ch.is_ascii_digit()) {
                    after_column
                } else {
                    after_line
                }
            } else {
                after_line
            };
            (candidate.parse::<usize>().ok(), text)
        } else if looks_like_search_path(path) {
            (None, rest)
        } else {
            return None;
        }
    } else if looks_like_search_path(path) {
        (None, rest)
    } else {
        return None;
    };

    Some(SearchMatch {
        path: path.trim().to_string(),
        line_number,
        text: text.trim().to_string(),
    })
}

pub(crate) fn parse_rg_json_match_line(line: &str) -> Option<SearchMatch> {
    if !looks_like_rg_json_line(line) || !json_field_has_string_value(line, "type", "match") {
        return None;
    }

    let path_section = line.split_once("\"path\"")?.1;
    let path = extract_json_string_field(path_section, "text")
        .or_else(|| extract_json_string_field(path_section, "path"))?;
    let lines_section = line.split_once("\"lines\"").map(|(_, section)| section);
    let text = lines_section
        .and_then(|section| extract_json_string_field(section, "text"))
        .unwrap_or_default();
    Some(SearchMatch {
        path: path.trim().to_string(),
        line_number: extract_json_usize_field(line, "line_number"),
        text: text.trim().to_string(),
    })
}

fn looks_like_rg_json_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('{')
        && trimmed.contains("\"type\"")
        && (trimmed.contains("\"data\"") || trimmed.contains("\"path\""))
}

fn json_field_has_string_value(line: &str, field: &str, value: &str) -> bool {
    extract_json_string_field(line, field).is_some_and(|found| found == value)
}

fn extract_json_string_field(input: &str, field: &str) -> Option<String> {
    let marker = format!("\"{field}\"");
    let after_marker = input.split_once(&marker)?.1;
    let after_colon = after_marker.split_once(':')?.1.trim_start();
    let value = after_colon.strip_prefix('"')?;
    parse_json_string_prefix(value)
}

fn extract_json_usize_field(input: &str, field: &str) -> Option<usize> {
    let marker = format!("\"{field}\"");
    let after_marker = input.split_once(&marker)?.1;
    let after_colon = after_marker.split_once(':')?.1.trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse::<usize>().ok())?
}

fn parse_json_string_prefix(input: &str) -> Option<String> {
    let mut output = String::new();
    let mut escaped = false;
    for ch in input.chars() {
        if escaped {
            match ch {
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                '"' => output.push('"'),
                '\\' => output.push('\\'),
                other => output.push(other),
            }
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => return Some(output),
            other => output.push(other),
        }
    }
    None
}

fn parse_search_heading_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed == "--"
        || trimmed.contains(':')
        || trimmed.contains("://")
        || trimmed.split_whitespace().count() > 1
    {
        return None;
    }
    parse_file_list_entry_line(trimmed).filter(|path| looks_like_search_path(path))
}

fn parse_heading_search_match_line(line: &str, path: Option<&str>) -> Option<SearchMatch> {
    let path = path?;
    let trimmed = line.trim_start();
    let (candidate, after_line) = trimmed.split_once(':')?;
    if !candidate.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let text = if let Some((column, after_column)) = after_line.split_once(':') {
        if column.chars().all(|ch| ch.is_ascii_digit()) {
            after_column
        } else {
            after_line
        }
    } else {
        after_line
    };
    Some(SearchMatch {
        path: path.to_string(),
        line_number: candidate.parse::<usize>().ok(),
        text: text.trim().to_string(),
    })
}

fn count_heading_search_matches(lines: &[&str]) -> usize {
    let mut count = 0usize;
    let mut current_path = None::<String>;
    for line in lines {
        if parse_search_match_line(line).is_some() || parse_rg_json_match_line(line).is_some() {
            current_path = None;
            continue;
        }
        if let Some(path) = parse_search_heading_line(line) {
            current_path = Some(path);
            continue;
        }
        if parse_heading_search_match_line(line, current_path.as_deref()).is_some() {
            count += 1;
        }
    }
    count
}

fn looks_like_search_path(path: &str) -> bool {
    path.contains('/') || path.contains('\\') || path.contains('.')
}

fn looks_like_file_list_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("[...")
        || trimmed.contains(" directories, ")
        || trimmed.contains("://")
    {
        return false;
    }
    trimmed.starts_with("./")
        || trimmed.starts_with('/')
        || trimmed.starts_with("|-- ")
        || trimmed.starts_with("`-- ")
        || trimmed.contains("\u{251c}\u{2500}\u{2500} ")
        || trimmed.contains("\u{2514}\u{2500}\u{2500} ")
        || (trimmed.contains('/') && !trimmed.contains("://") && !trimmed.contains(' '))
        || looks_like_bare_path_entry(trimmed)
}

pub(crate) fn parse_file_list_entry_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if looks_like_file_list_line(trimmed) {
        return Some(normalize_file_list_path(trimmed));
    }
    parse_ls_listing_path(trimmed)
}

fn looks_like_bare_path_entry(trimmed: &str) -> bool {
    if trimmed.is_empty()
        || trimmed.chars().any(char::is_whitespace)
        || trimmed.contains(':')
        || trimmed.starts_with('-')
        || trimmed.starts_with('{')
        || trimmed.starts_with('[')
    {
        return false;
    }

    if matches!(
        trimmed,
        "Cargo.toml"
            | "Cargo.lock"
            | "Makefile"
            | "README"
            | "README.md"
            | "LICENSE"
            | "AGENTS.md"
            | ".gitignore"
    ) {
        return true;
    }

    let Some((_, ext)) = trimmed.rsplit_once('.') else {
        return false;
    };
    !ext.is_empty()
        && ext.len() <= 12
        && ext
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn parse_ls_listing_path(trimmed: &str) -> Option<String> {
    if trimmed.is_empty() || trimmed.starts_with("total ") {
        return None;
    }
    let first = trimmed.chars().next()?;
    if !matches!(first, '-' | 'd' | 'l' | 'c' | 'b' | 'p' | 's' | 'D') {
        return None;
    }
    let parts = trimmed.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 9 || !looks_like_ls_mode(parts[0]) {
        return None;
    }
    let path = parts[8..].join(" ");
    let path = path.trim();
    if path.is_empty() || path == "." || path == ".." {
        None
    } else {
        Some(path.to_string())
    }
}

fn looks_like_ls_mode(mode: &str) -> bool {
    mode.len() >= 10
        && mode.chars().all(|ch| {
            matches!(
                ch,
                '-' | 'd'
                    | 'l'
                    | 'c'
                    | 'b'
                    | 'p'
                    | 's'
                    | 'D'
                    | 'r'
                    | 'w'
                    | 'x'
                    | 'S'
                    | 'T'
                    | 't'
                    | '+'
            )
        })
}

fn normalize_file_list_path(entry: &str) -> String {
    let trimmed = entry.trim();
    for marker in [
        "|-- ",
        "`-- ",
        "\u{251c}\u{2500}\u{2500} ",
        "\u{2514}\u{2500}\u{2500} ",
    ] {
        if let Some((_, path)) = trimmed.rsplit_once(marker) {
            return path.trim().to_string();
        }
    }
    trimmed.to_string()
}

fn top_level_path_segment(path: &str) -> String {
    let trimmed = path.trim_start_matches("./").trim_start_matches('/');
    trimmed
        .split('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or(".")
        .to_string()
}

fn path_extension_label(path: &str) -> String {
    let file_name = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches('/');
    file_name
        .rsplit_once('.')
        .and_then(|(_, ext)| {
            let valid = !ext.is_empty()
                && ext.len() <= 12
                && ext
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
            valid.then(|| ext.to_ascii_lowercase())
        })
        .unwrap_or_else(|| "none".to_string())
}

fn format_count_map(label: &str, counts: &BTreeMap<String, usize>, limit: usize) -> String {
    let mut entries = counts.iter().collect::<Vec<_>>();
    entries.sort_by_key(|(name, count)| (Reverse(**count), (*name).clone()));
    let mut rendered = entries
        .iter()
        .take(limit)
        .map(|(name, count)| format!("{name}={count}"))
        .collect::<Vec<_>>()
        .join(", ");
    if entries.len() > limit {
        rendered.push_str(&format!(" (+{} more)", entries.len() - limit));
    }
    format!("{label}: {rendered}")
}

fn is_critical_preserve_line(line: &str) -> bool {
    is_error_signal_line(line)
        || count_file_location_signals(line) > 0
        || is_diff_hunk_line(line)
        || is_test_failure_signal_line(line)
        || is_rust_exit_status_line(line)
        || is_stack_signal_line(line)
        || is_rust_diagnostic_signal_line(line)
        || is_log_level_signal_line(line)
}

pub(crate) fn is_log_level_signal_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    matches!(log_stream_level_label(line), Some("error" | "fatal"))
        || lower.starts_with("error ")
        || lower.starts_with("warn ")
        || lower.starts_with("warning ")
        || lower.starts_with("fatal ")
        || lower.starts_with("[error]")
        || lower.starts_with("[warn]")
        || lower.contains(" error ")
        || lower.contains(" fatal ")
        || lower.contains(" warn ")
}

fn log_stream_level_label(line: &str) -> Option<&'static str> {
    let lower = line.trim_start().to_ascii_lowercase();
    for (level, label) in [
        ("fatal", "fatal"),
        ("error", "error"),
        ("warn", "warn"),
        ("warning", "warn"),
        ("info", "info"),
        ("debug", "debug"),
        ("trace", "trace"),
    ] {
        if contains_json_log_level(&lower, level)
            || contains_kv_log_level(&lower, level)
            || starts_with_log_level_token(&lower, level)
            || contains_delimited_log_level(&lower, level)
        {
            return Some(label);
        }
    }
    None
}

fn contains_json_log_level(lower: &str, level: &str) -> bool {
    for field in ["level", "severity", "status"] {
        if lower.contains(&format!("\"{field}\":\"{level}\""))
            || lower.contains(&format!("\"{field}\": \"{level}\""))
        {
            return true;
        }
    }
    false
}

fn contains_kv_log_level(lower: &str, level: &str) -> bool {
    for field in ["level", "severity", "status"] {
        if lower.contains(&format!("{field}={level}"))
            || lower.contains(&format!("{field}: {level}"))
        {
            return true;
        }
    }
    false
}

fn starts_with_log_level_token(lower: &str, level: &str) -> bool {
    lower.starts_with(&format!("[{level}]"))
        || lower.starts_with(&format!("{level} "))
        || lower.starts_with(&format!("{level}:"))
        || lower.starts_with(&format!("{level}\t"))
}

fn contains_delimited_log_level(lower: &str, level: &str) -> bool {
    lower.contains(&format!(" {level} "))
        || lower.contains(&format!(" {level}:"))
        || lower.contains(&format!(" [{level}]"))
        || lower.contains(&format!(" {level}\t"))
}
