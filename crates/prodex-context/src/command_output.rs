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
mod classification;
mod critical_blocks;
mod diagnostics;
mod git_search;
mod git_search_parse;
mod intent;
mod kind_detection;
mod log_stream;
mod noisy_output;
mod noisy_success;
mod path_aliases;
mod rust_diagnostics;
mod structured_json;
mod success;
mod watcher_delta;

pub use intent::extract_intent_terms_from_prompt;
pub use kind_detection::infer_command_output_kind_from_metadata;
pub(crate) use log_stream::is_log_level_signal_line;
pub use success::compact_successful_command_output_with_options;

use ci_failure::compact_ci_failure_log_output;
use classification::*;
pub(crate) use critical_blocks::*;
use diagnostics::compact_diagnostic_output;
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
use noisy_output::compact_noisy_success_output;
pub(crate) use noisy_success::*;
use path_aliases::canonicalize_compacted_command_paths;
use rust_diagnostics::compact_rust_diagnostic_output;
use structured_json::compact_structured_json_output;
use success::{count_success_output_path_extensions, count_success_output_path_roots};
use watcher_delta::compact_watcher_delta_output;

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
