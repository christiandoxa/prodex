use crate::blob_noise::context_noise_strip_path_location_suffix_supplement;
use crate::critical_signal::{
    count_critical_signals, count_file_location_signals, critical_signal_self_check,
    is_diff_hunk_line, is_error_signal_line, is_rust_diagnostic_signal_line, is_stack_signal_line,
    is_test_failure_signal_line, looks_like_location_path,
};
use crate::estimate_context_tokens;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::BTreeMap;

mod critical_blocks;
mod git_search;
mod git_search_parse;
mod intent;
mod kind_detection;
mod structured_json;

pub use intent::extract_intent_terms_from_prompt;
pub use kind_detection::infer_command_output_kind_from_metadata;

pub(crate) use critical_blocks::*;
use git_search::{
    collect_file_list_entries, collect_search_output_matches, compact_file_list_output,
    compact_git_diff_output, compact_git_diff_output_with_intent, compact_git_log_stat_output,
    compact_git_status_output, compact_search_output,
};
pub(crate) use git_search_parse::*;
#[cfg(not(feature = "mojo"))]
use intent::intent_line_matches;
use intent::{
    compact_command_output_for_intent, ensure_no_critical_signal_loss_for_intent,
    normalize_intent_terms_with_prompt_expansion,
};
use kind_detection::detect_command_output_kind_with_hint;
#[cfg(not(feature = "mojo"))]
use kind_detection::{
    command_metadata_subcommand_after, command_metadata_token_command_name, command_metadata_tokens,
};
use structured_json::compact_structured_json_output;

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

#[derive(Debug, Clone, Copy)]
pub struct CommandOutputCompactLimits {
    pub kind: CommandOutputKind,
    pub max_lines: usize,
    pub head_lines: usize,
    pub tail_lines: usize,
    pub max_line_chars: usize,
    pub max_search_matches_per_file: usize,
    pub max_path_entries: usize,
}

impl CommandOutputCompactOptions {
    pub fn from_limits(limits: CommandOutputCompactLimits) -> Self {
        Self {
            kind: limits.kind,
            max_lines: limits.max_lines,
            head_lines: limits.head_lines,
            tail_lines: limits.tail_lines,
            max_line_chars: limits.max_line_chars,
            max_search_matches_per_file: limits.max_search_matches_per_file,
            max_path_entries: limits.max_path_entries,
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
    let structured_json_output =
        command_output_kind_allows_structured_json_compaction(detected_kind)
            .then(|| compact_structured_json_output(&normalized, options))
            .flatten();
    let output = structured_json_output.unwrap_or_else(|| match detected_kind {
        CommandOutputKind::GitStatus => compact_git_status_output(&normalized, options),
        CommandOutputKind::GitDiff => compact_git_diff_output(&normalized, options),
        CommandOutputKind::GitLog => compact_git_log_stat_output(&normalized, options),
        CommandOutputKind::Search => compact_search_output(&normalized, options),
        CommandOutputKind::FileList => compact_file_list_output(&normalized, options),
        CommandOutputKind::Auto
        | CommandOutputKind::RustDiagnostics
        | CommandOutputKind::Diagnostics
        | CommandOutputKind::LogStream
        | CommandOutputKind::NoisySuccess
        | CommandOutputKind::Plain => smart_truncate_command_output(&normalized, options),
    });

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
    report.compacted_lines = count_text_lines(&output);
    report.estimated_tokens_after =
        estimate_context_tokens(output.chars().count(), output.split_whitespace().count());
    report.output = output;
    report
}
