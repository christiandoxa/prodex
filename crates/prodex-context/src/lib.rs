use anyhow::{Context, Result};
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use terminal_ui::{fit_cell, section_header, section_header_with_width};

const CONTEXT_AUDIT_ROOTS: &[&str] = &[
    "AGENTS.md",
    "AGENTS.override.md",
    "memories",
    "memories_extensions",
    "rules",
    "skills",
];

#[derive(Debug, Clone, Serialize)]
pub struct ContextAuditEntry {
    pub path: PathBuf,
    pub relative_path: String,
    pub bytes: u64,
    pub chars: usize,
    pub words: usize,
    pub estimated_tokens: usize,
    pub compressible: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextAuditReport {
    pub root: PathBuf,
    pub files: Vec<ContextAuditEntry>,
    pub total_bytes: u64,
    pub total_chars: usize,
    pub total_words: usize,
    pub total_estimated_tokens: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextCompressEntry {
    pub path: PathBuf,
    pub backup_path: Option<PathBuf>,
    pub status: String,
    pub original_bytes: u64,
    pub compressed_bytes: u64,
    pub estimated_tokens_before: usize,
    pub estimated_tokens_after: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextCompressReport {
    pub entries: Vec<ContextCompressEntry>,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextBlobNoiseKind {
    Base64Blob,
    MinifiedJsJson,
    LockfileOrVendor,
    BinaryText,
    RepeatedPathFlood,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextBlobNoiseFinding {
    pub kind: ContextBlobNoiseKind,
    pub line: Option<usize>,
    pub bytes: usize,
    pub score: usize,
    pub detail: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ContextBlobNoiseReport {
    pub bytes: usize,
    pub lines: usize,
    pub findings: Vec<ContextBlobNoiseFinding>,
}

impl ContextBlobNoiseReport {
    pub fn is_noise(&self) -> bool {
        !self.findings.is_empty()
    }

    pub fn has_kind(&self, kind: ContextBlobNoiseKind) -> bool {
        self.findings.iter().any(|finding| finding.kind == kind)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct CriticalSignalCounts {
    pub errors: usize,
    pub file_locations: usize,
    pub diff_hunks: usize,
    pub test_failures: usize,
    pub exit_codes: usize,
    pub stack_markers: usize,
    pub rust_diagnostics: usize,
}

impl CriticalSignalCounts {
    pub fn total(self) -> usize {
        self.errors
            + self.file_locations
            + self.diff_hunks
            + self.test_failures
            + self.exit_codes
            + self.stack_markers
            + self.rust_diagnostics
    }

    pub fn is_empty(self) -> bool {
        self.total() == 0
    }

    fn saturating_loss(self, after: Self) -> Self {
        Self {
            errors: self.errors.saturating_sub(after.errors),
            file_locations: self.file_locations.saturating_sub(after.file_locations),
            diff_hunks: self.diff_hunks.saturating_sub(after.diff_hunks),
            test_failures: self.test_failures.saturating_sub(after.test_failures),
            exit_codes: self.exit_codes.saturating_sub(after.exit_codes),
            stack_markers: self.stack_markers.saturating_sub(after.stack_markers),
            rust_diagnostics: self.rust_diagnostics.saturating_sub(after.rust_diagnostics),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CriticalSignalSelfCheck {
    pub before: CriticalSignalCounts,
    pub after: CriticalSignalCounts,
    pub lost: CriticalSignalCounts,
    pub gained: CriticalSignalCounts,
}

impl CriticalSignalSelfCheck {
    pub fn passed(self) -> bool {
        self.lost.is_empty()
    }

    pub fn has_loss(self) -> bool {
        !self.passed()
    }
}

pub fn count_critical_signals(input: &str) -> CriticalSignalCounts {
    let normalized = normalize_command_output(input);
    let mut counts = CriticalSignalCounts::default();

    for line in command_lines(&normalized) {
        if is_error_signal_line(line) {
            counts.errors += 1;
        }
        counts.file_locations += count_file_location_signals(line);
        if is_diff_hunk_line(line) {
            counts.diff_hunks += 1;
        }
        if is_test_failure_signal_line(line) {
            counts.test_failures += 1;
        }
        if is_rust_exit_status_line(line) {
            counts.exit_codes += 1;
        }
        if is_stack_signal_line(line) {
            counts.stack_markers += 1;
        }
        if is_rust_diagnostic_signal_line(line) {
            counts.rust_diagnostics += 1;
        }
    }

    counts
}

pub fn critical_signal_self_check(before: &str, after: &str) -> CriticalSignalSelfCheck {
    let before = count_critical_signals(before);
    let after = count_critical_signals(after);
    CriticalSignalSelfCheck {
        before,
        after,
        lost: before.saturating_loss(after),
        gained: after.saturating_loss(before),
    }
}

pub fn detect_context_blob_noise(input: &str) -> ContextBlobNoiseReport {
    detect_context_blob_noise_inner(None, input)
}

pub fn detect_context_blob_noise_for_path(path: &Path, input: &str) -> ContextBlobNoiseReport {
    detect_context_blob_noise_inner(Some(path), input)
}

pub fn is_context_blob_noise(input: &str) -> bool {
    detect_context_blob_noise(input).is_noise()
}

fn detect_context_blob_noise_inner(path: Option<&Path>, input: &str) -> ContextBlobNoiseReport {
    let normalized = normalize_command_output(input);
    let lines = command_lines(&normalized);
    let mut findings = Vec::new();

    if path.is_some_and(is_lockfile_or_vendor_path) {
        findings.push(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: None,
            bytes: input.len(),
            score: input.len().min(usize::MAX / 2),
            detail: "path looks like generated dependency/vendor content".to_string(),
        });
    }

    if input.chars().any(|ch| ch == '\0' || ch == '\u{fffd}') {
        findings.push(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::BinaryText,
            line: None,
            bytes: input.len(),
            score: input.len(),
            detail: "text contains binary replacement or NUL characters".to_string(),
        });
    }

    for (index, line) in lines.iter().enumerate() {
        let line_number = Some(index + 1);
        let trimmed = line.trim();
        if trimmed.len() >= 512 && looks_like_base64_blob(trimmed) {
            findings.push(ContextBlobNoiseFinding {
                kind: ContextBlobNoiseKind::Base64Blob,
                line: line_number,
                bytes: trimmed.len(),
                score: trimmed.len(),
                detail: "long high-entropy base64-like line".to_string(),
            });
        }
        if trimmed.len() >= 800 && looks_like_minified_js_json(trimmed) {
            findings.push(ContextBlobNoiseFinding {
                kind: ContextBlobNoiseKind::MinifiedJsJson,
                line: line_number,
                bytes: trimmed.len(),
                score: trimmed.len(),
                detail: "long minified JSON/JavaScript-like line".to_string(),
            });
        }
    }

    if let Some((line, count)) = repeated_path_flood(&lines) {
        findings.push(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::RepeatedPathFlood,
            line: Some(line),
            bytes: input.len(),
            score: count,
            detail: format!("{count} path-like lines detected"),
        });
    }

    add_context_blob_noise_supplemental_findings(path, input, &lines, &mut findings);

    ContextBlobNoiseReport {
        bytes: input.len(),
        lines: count_text_lines(&normalized),
        findings,
    }
}

pub fn collect_context_audit_report(root: &Path, limit: usize) -> Result<ContextAuditReport> {
    let mut paths = Vec::new();
    for entry in CONTEXT_AUDIT_ROOTS {
        collect_context_files(&root.join(entry), &mut paths)?;
    }
    paths.sort();
    paths.dedup();

    let mut files = Vec::new();
    for path in paths {
        if !is_auditable_context_file(&path) {
            continue;
        }
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => continue,
        };
        let metadata =
            fs::metadata(&path).with_context(|| format!("failed to inspect {}", path.display()))?;
        let chars = text.chars().count();
        let words = text.split_whitespace().count();
        let estimated_tokens = estimate_context_tokens(chars, words);
        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(path.as_path())
            .display()
            .to_string();
        let compressible = is_compressible_context_file(&path);
        files.push(ContextAuditEntry {
            path,
            relative_path,
            bytes: metadata.len(),
            chars,
            words,
            estimated_tokens,
            compressible,
        });
        if limit > 0 && files.len() > limit.saturating_mul(16) {
            files.sort_by_key(|entry| Reverse(entry.estimated_tokens));
            files.truncate(limit.saturating_mul(8).max(limit));
        }
    }

    files.sort_by_key(|entry| Reverse(entry.estimated_tokens));

    let total_bytes = files.iter().map(|entry| entry.bytes).sum();
    let total_chars = files.iter().map(|entry| entry.chars).sum();
    let total_words = files.iter().map(|entry| entry.words).sum();
    let total_estimated_tokens = files.iter().map(|entry| entry.estimated_tokens).sum();

    Ok(ContextAuditReport {
        root: root.to_path_buf(),
        files,
        total_bytes,
        total_chars,
        total_words,
        total_estimated_tokens,
    })
}

pub fn compress_context_path(path: &Path, dry_run: bool) -> Result<ContextCompressReport> {
    let mut paths = Vec::new();
    collect_context_files(path, &mut paths)?;
    paths.sort();
    paths.dedup();

    let mut entries = Vec::new();
    for path in paths {
        entries.push(compress_context_file(&path, dry_run)?);
    }
    Ok(ContextCompressReport { entries })
}

fn compress_context_file(path: &Path, dry_run: bool) -> Result<ContextCompressEntry> {
    if !is_compressible_context_file(path) {
        return Ok(ContextCompressEntry {
            path: path.to_path_buf(),
            backup_path: None,
            status: "skipped_not_prose".to_string(),
            original_bytes: 0,
            compressed_bytes: 0,
            estimated_tokens_before: 0,
            estimated_tokens_after: 0,
        });
    }

    let original = fs::read_to_string(path)
        .with_context(|| format!("failed to read context file {}", path.display()))?;
    let compressed = compress_context_text(&original);
    let original_bytes = original.len() as u64;
    let compressed_bytes = compressed.len() as u64;
    let estimated_tokens_before = estimate_context_tokens(
        original.chars().count(),
        original.split_whitespace().count(),
    );
    let estimated_tokens_after = estimate_context_tokens(
        compressed.chars().count(),
        compressed.split_whitespace().count(),
    );
    let backup_path = context_backup_path(path);

    if backup_path.exists() {
        return Ok(ContextCompressEntry {
            path: path.to_path_buf(),
            backup_path: Some(backup_path),
            status: "skipped_backup_exists".to_string(),
            original_bytes,
            compressed_bytes,
            estimated_tokens_before,
            estimated_tokens_after,
        });
    }

    if compressed_bytes >= original_bytes {
        return Ok(ContextCompressEntry {
            path: path.to_path_buf(),
            backup_path: Some(backup_path),
            status: "skipped_no_gain".to_string(),
            original_bytes,
            compressed_bytes,
            estimated_tokens_before,
            estimated_tokens_after,
        });
    }

    if !dry_run {
        fs::write(&backup_path, &original)
            .with_context(|| format!("failed to write backup {}", backup_path.display()))?;
        fs::write(path, &compressed)
            .with_context(|| format!("failed to write compressed context {}", path.display()))?;
    }

    Ok(ContextCompressEntry {
        path: path.to_path_buf(),
        backup_path: Some(backup_path),
        status: if dry_run {
            "dry_run".to_string()
        } else {
            "compressed".to_string()
        },
        original_bytes,
        compressed_bytes,
        estimated_tokens_before,
        estimated_tokens_after,
    })
}

fn collect_context_files(path: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_file() {
        paths.push(path.to_path_buf());
        return Ok(());
    }
    if !path.is_dir() {
        return Ok(());
    }

    let mut entries = fs::read_dir(path)
        .with_context(|| format!("failed to read context directory {}", path.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("failed to read entry in {}", path.display()))?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        collect_context_files(&entry.path(), paths)?;
    }
    Ok(())
}

pub fn render_context_audit_report_with_width(
    report: &ContextAuditReport,
    limit: usize,
    total_width: usize,
) -> String {
    let mut lines = vec![section_header_with_width("Context Audit", total_width)];
    lines.push(format!("Root: {}", report.root.display()));
    lines.push(format!(
        "Approx tokens: {} across {} files ({} bytes, {} words)",
        format_count(report.total_estimated_tokens),
        report.files.len(),
        format_count(report.total_bytes),
        format_count(report.total_words),
    ));

    if report.files.is_empty() {
        lines.push("No context files found.".to_string());
        return lines.join("\n");
    }

    lines.push(String::new());
    lines.push(format!(
        "{:>8}  {:>8}  {:>8}  {:>4}  {}",
        "tokens", "bytes", "words", "cmp", "file"
    ));
    let file_width = total_width.saturating_sub(38).max(20);
    let shown = if limit == 0 {
        report.files.len()
    } else {
        report.files.len().min(limit)
    };
    for entry in report.files.iter().take(shown) {
        lines.push(format!(
            "{:>8}  {:>8}  {:>8}  {:>4}  {}",
            format_count(entry.estimated_tokens),
            format_count(entry.bytes),
            format_count(entry.words),
            if entry.compressible { "yes" } else { "no" },
            fit_cell(&entry.relative_path, file_width),
        ));
    }
    if shown < report.files.len() {
        lines.push(format!(
            "... {} more files hidden",
            report.files.len() - shown
        ));
    }
    lines.join("\n")
}

pub fn render_context_compress_report(report: &ContextCompressReport, dry_run: bool) -> String {
    let title = if dry_run {
        "Context Compress Dry Run"
    } else {
        "Context Compress"
    };
    let mut lines = vec![section_header(title)];
    if report.entries.is_empty() {
        lines.push("No files matched.".to_string());
        return lines.join("\n");
    }

    for entry in &report.entries {
        let saved = entry.original_bytes.saturating_sub(entry.compressed_bytes);
        let token_saved = entry
            .estimated_tokens_before
            .saturating_sub(entry.estimated_tokens_after);
        lines.push(format!(
            "{}: {} ({} bytes saved, ~{} tokens saved)",
            entry.status,
            entry.path.display(),
            format_count(saved),
            format_count(token_saved),
        ));
        if let Some(backup_path) = &entry.backup_path
            && entry.status == "compressed"
        {
            lines.push(format!("Backup: {}", backup_path.display()));
        }
    }
    lines.join("\n")
}

pub fn compact_command_output(input: &str, kind: CommandOutputKind) -> String {
    let options = CommandOutputCompactOptions {
        kind,
        ..CommandOutputCompactOptions::default()
    };
    compact_command_output_with_options(input, &options).output
}

pub fn compact_command_output_with_options(
    input: &str,
    options: &CommandOutputCompactOptions,
) -> CommandOutputCompactReport {
    let normalized = normalize_command_output(input);
    let detected_kind = match options.kind {
        CommandOutputKind::Auto => detect_command_output_kind(&normalized),
        explicit => explicit,
    };
    let output = match detected_kind {
        CommandOutputKind::Auto => smart_truncate_command_output(&normalized, options),
        CommandOutputKind::GitStatus => compact_git_status_output(&normalized, options),
        CommandOutputKind::GitDiff => compact_git_diff_output(&normalized, options),
        CommandOutputKind::RustDiagnostics => compact_rust_diagnostic_output(&normalized, options),
        CommandOutputKind::Diagnostics => compact_diagnostic_output(&normalized, options),
        CommandOutputKind::GitLog => compact_git_log_stat_output(&normalized, options),
        CommandOutputKind::Search => compact_search_output(&normalized, options),
        CommandOutputKind::FileList => compact_file_list_output(&normalized, options),
        CommandOutputKind::NoisySuccess => compact_noisy_success_output(&normalized, options),
        CommandOutputKind::Plain => smart_truncate_command_output(&normalized, options),
    };

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

fn compact_git_status_output(input: &str, options: &CommandOutputCompactOptions) -> String {
    let mut summary = GitStatusSummary::default();
    let lines = command_lines(input);
    let short_format = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .all(|line| is_short_git_status_line(line) || line.starts_with("## "));

    if short_format {
        for line in lines {
            parse_short_git_status_line(line, &mut summary);
        }
    } else {
        parse_long_git_status_lines(&lines, &mut summary);
    }

    let category_limit = options.max_path_entries.max(1).div_ceil(6).max(4);
    let mut output = Vec::new();
    output.push("git status summary".to_string());
    if let Some(branch) = summary.branch {
        output.push(format!("branch: {branch}"));
    }
    if summary.clean {
        output.push("clean: true".to_string());
    }
    push_item_summary(&mut output, "staged", &summary.staged, category_limit);
    push_item_summary(&mut output, "modified", &summary.modified, category_limit);
    push_item_summary(&mut output, "deleted", &summary.deleted, category_limit);
    push_item_summary(&mut output, "renamed", &summary.renamed, category_limit);
    push_item_summary(
        &mut output,
        "conflicted",
        &summary.conflicted,
        category_limit,
    );
    push_item_summary(&mut output, "untracked", &summary.untracked, category_limit);
    push_item_summary(&mut output, "other", &summary.other, category_limit);

    if output.len() == 1 {
        return smart_truncate_command_output(input, options);
    }

    finalize_compacted_command_output(CommandOutputKind::GitStatus, input, output, options)
}

fn compact_git_diff_output(input: &str, options: &CommandOutputCompactOptions) -> String {
    let lines = command_lines(input);
    let sections = split_git_diff_sections(&lines);
    if sections.is_empty() {
        if let Some(output) = compact_git_diff_stat_output(&lines, options) {
            return finalize_compacted_command_output(
                CommandOutputKind::GitDiff,
                input,
                output,
                options,
            );
        }
        return smart_truncate_command_output(input, options);
    }

    let summaries = sections
        .iter()
        .map(|section| summarize_git_diff_section(section))
        .collect::<Vec<_>>();
    let total_added = summaries.iter().map(|summary| summary.added).sum::<usize>();
    let total_removed = summaries
        .iter()
        .map(|summary| summary.removed)
        .sum::<usize>();
    let total_hunks = summaries.iter().map(|summary| summary.hunks).sum::<usize>();

    let summary_line_count = summaries.len().saturating_add(4);
    let detail_budget = options
        .max_lines
        .saturating_sub(summary_line_count)
        .saturating_sub(4);
    let per_file_budget = detail_budget
        .checked_div(sections.len().max(1))
        .unwrap_or(0)
        .max(6);

    let mut output = Vec::new();
    output.push(format!(
        "git diff summary: {} files, +{}, -{}, {} hunks",
        summaries.len(),
        total_added,
        total_removed,
        total_hunks,
    ));
    for summary in &summaries {
        let binary = if summary.binary { ", binary" } else { "" };
        output.push(format!(
            "{}: +{}, -{}, {} hunks{}",
            summary.path, summary.added, summary.removed, summary.hunks, binary,
        ));
    }
    output.push(String::new());
    output.push("diff excerpts:".to_string());

    for (section, summary) in sections.iter().zip(summaries.iter()) {
        let mut kept_detail = 0usize;
        let mut omitted_detail = 0usize;
        for line in section {
            if is_git_diff_structural_line(line) {
                output.push(truncate_command_line(line, options.max_line_chars));
            } else if kept_detail < per_file_budget {
                output.push(truncate_command_line(line, options.max_line_chars));
                kept_detail += 1;
            } else {
                omitted_detail += 1;
            }
        }
        if omitted_detail > 0 {
            output.push(format!(
                "[... omitted {} diff lines for {} ...]",
                omitted_detail, summary.path,
            ));
        }
    }

    finalize_compacted_command_output(CommandOutputKind::GitDiff, input, output, options)
}

fn compact_git_log_stat_output(input: &str, options: &CommandOutputCompactOptions) -> String {
    let lines = command_lines(input);
    let commits = parse_git_log_stat_commits(&lines);
    if commits.is_empty() {
        return smart_truncate_command_output(input, options);
    }

    let stat_files = commits
        .iter()
        .map(|commit| commit.stat_lines.len())
        .sum::<usize>();
    let mut output = Vec::new();
    output.push(format!(
        "git log --stat summary: {} commits, {} stat file entries",
        commits.len(),
        stat_files,
    ));

    let commit_limit = options
        .max_lines
        .max(24)
        .saturating_div(8)
        .max(2)
        .min(commits.len());
    let per_commit_stat_limit = options
        .max_path_entries
        .max(1)
        .checked_div(commit_limit.max(1))
        .unwrap_or(1)
        .clamp(2, 8);

    for commit in commits.iter().take(commit_limit) {
        output.push(format!(
            "commit: {}",
            truncate_command_line(&commit.header, options.max_line_chars)
        ));
        for meta in commit.metadata.iter().take(2) {
            output.push(format!(
                "  {}",
                truncate_command_line(meta, options.max_line_chars)
            ));
        }
        for subject in commit.subject.iter().take(2) {
            output.push(format!(
                "  subject: {}",
                truncate_command_line(subject, options.max_line_chars)
            ));
        }
        for summary in &commit.stat_summaries {
            output.push(format!(
                "  stat totals: {}",
                truncate_command_line(summary, options.max_line_chars)
            ));
        }
        if !commit.stat_lines.is_empty() {
            output.push(format!("  stat files ({}):", commit.stat_lines.len()));
            push_head_tail_lines(
                &mut output,
                &commit.stat_lines,
                per_commit_stat_limit,
                options.max_line_chars,
                "stat entries",
                "    ",
            );
        }
    }

    if commits.len() > commit_limit {
        output.push(format!(
            "[... omitted {} commits ...]",
            commits.len() - commit_limit
        ));
    }

    finalize_compacted_command_output(CommandOutputKind::GitLog, input, output, options)
}

fn compact_search_output(input: &str, options: &CommandOutputCompactOptions) -> String {
    let lines = command_lines(input);
    let mut files: BTreeMap<String, Vec<SearchMatch>> = BTreeMap::new();
    let mut other = Vec::new();
    let mut current_heading_path = None::<String>;

    for line in lines {
        if let Some(search_match) =
            parse_rg_json_match_line(line).or_else(|| parse_search_match_line(line))
        {
            current_heading_path = Some(search_match.path.clone());
            files
                .entry(search_match.path.clone())
                .or_default()
                .push(search_match);
        } else if let Some(search_match) =
            parse_heading_search_match_line(line, current_heading_path.as_deref())
        {
            files
                .entry(search_match.path.clone())
                .or_default()
                .push(search_match);
        } else if let Some(path) = parse_search_heading_line(line) {
            current_heading_path = Some(path);
        } else if looks_like_rg_json_line(line) {
            // Non-match rg JSON records are command metadata, not useful context.
        } else if !line.trim().is_empty() {
            other.push(line.to_string());
        }
    }

    let total_matches = files.values().map(Vec::len).sum::<usize>();
    if total_matches == 0 {
        return smart_truncate_command_output(input, options);
    }

    let mut output = Vec::new();
    output.push(format!(
        "search summary: {} matches across {} files",
        total_matches,
        files.len(),
    ));
    for (path, matches) in files {
        output.push(format!("{path} ({} matches):", matches.len()));
        for search_match in matches
            .iter()
            .take(options.max_search_matches_per_file.max(1))
        {
            let prefix = search_match
                .line_number
                .map(|line| format!("{line}: "))
                .unwrap_or_default();
            output.push(format!(
                "  {}{}",
                prefix,
                truncate_command_line(&search_match.text, options.max_line_chars),
            ));
        }
        if matches.len() > options.max_search_matches_per_file.max(1) {
            output.push(format!(
                "  [... {} more matches in this file ...]",
                matches.len() - options.max_search_matches_per_file.max(1),
            ));
        }
    }

    if !other.is_empty() {
        output.push(format!("other lines ({}):", other.len()));
        for line in other.iter().take(4) {
            output.push(format!(
                "  {}",
                truncate_command_line(line, options.max_line_chars)
            ));
        }
        if other.len() > 4 {
            output.push(format!("  [... {} more other lines ...]", other.len() - 4));
        }
    }

    finalize_compacted_command_output(CommandOutputKind::Search, input, output, options)
}

fn compact_file_list_output(input: &str, options: &CommandOutputCompactOptions) -> String {
    let lines = command_lines(input);
    let mut entries = Vec::new();
    for line in lines {
        if let Some(entry) = parse_file_list_entry_line(line) {
            entries.push(entry);
        }
    }

    if entries.is_empty() {
        return smart_truncate_command_output(input, options);
    }

    let mut roots = BTreeMap::<String, usize>::new();
    let mut extensions = BTreeMap::<String, usize>::new();
    for entry in &entries {
        let path = normalize_file_list_path(entry);
        *roots.entry(top_level_path_segment(&path)).or_default() += 1;
        *extensions.entry(path_extension_label(&path)).or_default() += 1;
    }

    let mut output = Vec::new();
    output.push(format!("file list summary: {} entries", entries.len()));
    output.push(format_count_map("top roots", &roots, 8));
    output.push(format_count_map("extensions", &extensions, 8));
    output.push("entries:".to_string());

    let max_entries = options.max_path_entries.max(1);
    let head = max_entries.div_ceil(2);
    let tail = max_entries.saturating_sub(head);
    if entries.len() <= max_entries {
        for entry in entries {
            output.push(truncate_command_line(&entry, options.max_line_chars));
        }
    } else {
        for entry in entries.iter().take(head) {
            output.push(truncate_command_line(entry, options.max_line_chars));
        }
        output.push(format!(
            "[... omitted {} file-list entries ...]",
            entries.len().saturating_sub(head + tail),
        ));
        for entry in entries.iter().skip(entries.len().saturating_sub(tail)) {
            output.push(truncate_command_line(entry, options.max_line_chars));
        }
    }

    finalize_compacted_command_output(CommandOutputKind::FileList, input, output, options)
}

fn compact_git_diff_stat_output(
    lines: &[&str],
    options: &CommandOutputCompactOptions,
) -> Option<Vec<String>> {
    let stat_lines = lines
        .iter()
        .filter(|line| looks_like_git_diff_stat_line(line))
        .copied()
        .collect::<Vec<_>>();
    if stat_lines.is_empty()
        || !lines
            .iter()
            .any(|line| looks_like_git_diff_stat_summary(line))
    {
        return None;
    }

    let summary_lines = lines
        .iter()
        .filter(|line| looks_like_git_diff_stat_summary(line))
        .copied()
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    output.push(format!(
        "git diff summary: stat-only, {} file entries",
        stat_lines.len()
    ));
    for line in summary_lines {
        output.push(format!(
            "stat totals: {}",
            truncate_command_line(line.trim(), options.max_line_chars)
        ));
    }
    output.push("stat files:".to_string());

    let limit = options
        .max_path_entries
        .max(1)
        .min(options.max_lines.max(8));
    let head = limit.div_ceil(2);
    let tail = limit.saturating_sub(head);
    if stat_lines.len() <= limit {
        for line in stat_lines {
            output.push(format!(
                "  {}",
                truncate_command_line(line.trim(), options.max_line_chars)
            ));
        }
    } else {
        for line in stat_lines.iter().take(head) {
            output.push(format!(
                "  {}",
                truncate_command_line(line.trim(), options.max_line_chars)
            ));
        }
        output.push(format!(
            "  [... omitted {} stat entries ...]",
            stat_lines.len().saturating_sub(head + tail)
        ));
        for line in stat_lines
            .iter()
            .skip(stat_lines.len().saturating_sub(tail))
        {
            output.push(format!(
                "  {}",
                truncate_command_line(line.trim(), options.max_line_chars)
            ));
        }
    }

    Some(output)
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
        "rust/cargo summary: errors={}, warnings={}, failed_tests={}, panics={}, exit_statuses={}, noisy_success_lines={}",
        summary.errors,
        summary.warnings,
        summary.failed_tests.len(),
        summary.panics,
        summary.exit_statuses.len(),
        noise_counts.values().sum::<usize>(),
    ));
    if !noise_counts.is_empty() {
        output.push(format_count_map("noise", &noise_counts, 10));
    }
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
        }
        index += 1;
    }

    if summary.is_empty() && noise_counts.is_empty() && key_lines.is_empty() && blocks.is_empty() {
        return smart_truncate_command_output(input, options);
    }

    let mut output = Vec::new();
    output.push(format!(
        "diagnostic summary: errors={}, failed_tests={}, locations={}, stack_markers={}, exit_statuses={}, noisy_success_lines={}",
        summary.errors,
        summary.failed_tests.len(),
        summary.locations.len(),
        summary.stack_markers,
        summary.exit_statuses.len(),
        noise_counts.values().sum::<usize>(),
    ));
    if !noise_counts.is_empty() {
        output.push(format_count_map("noise", &noise_counts, 10));
    }
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

    finalize_compacted_command_output(CommandOutputKind::Diagnostics, input, output, options)
}

fn compact_noisy_success_output(input: &str, options: &CommandOutputCompactOptions) -> String {
    let lines = command_lines(input);
    if lines.is_empty() {
        return String::new();
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
        "success output summary: noisy_success_lines={}, critical_lines={}",
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
    let log_level_lines = lines
        .iter()
        .filter(|line| {
            let lower = line.trim_start().to_ascii_lowercase();
            contains_json_log_level(&lower, "info")
                || contains_json_log_level(&lower, "debug")
                || contains_json_log_level(&lower, "trace")
                || contains_json_log_level(&lower, "error")
                || contains_json_log_level(&lower, "fatal")
                || contains_json_log_level(&lower, "warn")
        })
        .count();
    log_level_lines.saturating_mul(2) >= non_empty
}

fn looks_like_noisy_success_output(lines: &[&str]) -> bool {
    let non_empty = lines.iter().filter(|line| !line.trim().is_empty()).count();
    if non_empty < 8 {
        return false;
    }
    if lines
        .iter()
        .any(|line| is_error_signal_line(line) || is_test_failure_signal_line(line))
    {
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
    if trimmed.starts_with("Compiling ") {
        Some("compiling")
    } else if trimmed.starts_with("Checking ") {
        Some("checking")
    } else if trimmed.starts_with("Fresh ") {
        Some("fresh")
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
    if trimmed.starts_with("PASS ") {
        Some("passed_suites")
    } else if lower.starts_with("test suites:") && lower.contains("passed") {
        Some("test_suites")
    } else if lower.starts_with("tests:") && lower.contains("passed") {
        Some("test_cases")
    } else if lower.starts_with("snapshots:") && lower.contains("passed") {
        Some("snapshots")
    } else if lower.starts_with("time:") {
        Some("test_time")
    } else if lower.starts_with("ran all test suites") {
        Some("test_runner_summary")
    } else if lower.starts_with("done in ") {
        Some("done")
    } else if lower.starts_with("added ") && lower.contains(" package") {
        Some("packages_added")
    } else if lower.starts_with("audited ") && lower.contains(" package") {
        Some("packages_audited")
    } else if lower == "up to date" || lower.starts_with("up to date in ") {
        Some("packages_up_to_date")
    } else if lower.starts_with("found 0 vulnerabilities") {
        Some("vulnerability_summary")
    } else if lower.starts_with("all files pass") {
        Some("formatter_summary")
    } else if lower.starts_with("built in ") || lower.contains(" built in ") {
        Some("build_summary")
    } else if lower.starts_with("compiled successfully") {
        Some("compile_summary")
    } else if lower.starts_with("tests/") && lower.contains(" passed") {
        Some("passed_tests")
    } else if lower.contains("::test_") && lower.ends_with(" passed") {
        Some("passed_tests")
    } else if is_pytest_progress_line(trimmed) {
        Some("pytest_progress")
    } else {
        None
    }
}

fn is_pytest_progress_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= 8
        && trimmed
            .chars()
            .all(|ch| matches!(ch, '.' | 's' | 'S' | 'x' | 'X'))
        && trimmed.chars().any(|ch| ch == '.')
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
        || lower.starts_with("ran all test suites")
        || lower.starts_with("found 0 vulnerabilities")
        || lower.contains(" passed in ")
}

fn is_diagnostic_failure_summary_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    lower.starts_with("test suites:") && lower.contains("failed")
        || lower.starts_with("tests:") && lower.contains("failed")
        || lower.contains(" failed, ")
        || lower.starts_with("failed ")
        || lower.starts_with("error summary")
}

fn is_noisy_success_key_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    lower.starts_with("test suites:")
        || lower.starts_with("tests:")
        || lower.starts_with("snapshots:")
        || lower.starts_with("ran all test suites")
        || lower.starts_with("done in ")
        || lower.starts_with("added ")
        || lower.starts_with("audited ")
        || lower.starts_with("found 0 vulnerabilities")
        || lower == "up to date"
        || lower.starts_with("up to date in ")
        || lower.starts_with("all files pass")
        || lower.starts_with("built in ")
        || lower.contains(" passed in ")
        || lower.starts_with("compiled successfully")
}

fn is_rust_success_summary_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("Finished ") || trimmed.starts_with("test result: ok")
}

fn rust_diagnostic_severity(line: &str) -> Option<RustDiagnosticSeverity> {
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

fn rust_failed_test_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("test ")?;
    let (name, status) = rest.rsplit_once(" ... ")?;
    (status == "FAILED").then_some(name.trim())
}

fn rust_failure_separator_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix("---- ")?.strip_suffix(" ----")?.trim();
    let name = inner
        .strip_suffix(" stdout")
        .or_else(|| inner.strip_suffix(" stderr"))
        .unwrap_or(inner)
        .trim();
    (!name.is_empty()).then_some(name)
}

fn generic_failed_test_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
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
    if trimmed.starts_with("Test Suites:") && trimmed.contains("failed") {
        return Some(trimmed);
    }
    if trimmed.starts_with("Tests:") && trimmed.contains("failed") {
        return Some(trimmed);
    }
    if trimmed.contains(" failed, ") || trimmed.starts_with("failed ") {
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

fn is_rust_panic_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.contains("panicked at ") || trimmed.contains("panicked at:")
}

fn is_rust_backtrace_start(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed == "stack backtrace:" || trimmed == "Backtrace:"
}

fn is_rust_exit_status_line(line: &str) -> bool {
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

fn is_rust_failure_summary_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("test result: FAILED")
        || trimmed == "failures:"
        || trimmed.starts_with("failures:")
        || trimmed.starts_with("error: aborting")
}

fn is_diagnostic_block_start(line: &str) -> bool {
    is_typescript_diagnostic_line(line)
        || is_exception_signal_line(line)
        || is_node_stack_error_line(line)
        || is_test_failure_signal_line(line)
        || is_stack_signal_line(line)
        || is_error_signal_line(line)
}

fn is_diagnostic_detection_start(line: &str) -> bool {
    is_typescript_diagnostic_line(line)
        || is_exception_signal_line(line)
        || is_node_stack_error_line(line)
        || is_test_failure_signal_line(line)
        || is_stack_signal_line(line)
        || line
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("npm err!")
}

fn is_typescript_diagnostic_line(line: &str) -> bool {
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

fn is_exception_signal_line(line: &str) -> bool {
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

    if max_lines < 8 {
        let mut output = Vec::new();
        output.push(format!(
            "command output summary: {} lines, {} critical lines preserved",
            lines.len(),
            critical.len()
        ));
        for (index, line) in critical.iter().take(max_lines.saturating_sub(1).max(1)) {
            output.push(format!(
                "L{}: {}",
                index + 1,
                truncate_command_line(line.trim(), options.max_line_chars)
            ));
        }
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
        "command output summary: {} lines, {} critical lines preserved",
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

    let head = budget.div_ceil(2).saturating_sub(1).max(1);
    let tail = budget.saturating_sub(head).saturating_sub(1);
    for (index, line) in critical.iter().take(head) {
        output.push(format!(
            "  L{}: {}",
            index + 1,
            truncate_command_line(line.trim(), options.max_line_chars)
        ));
    }
    output.push(format!(
        "  [... omitted {} critical lines ...]",
        critical.len().saturating_sub(head + tail)
    ));
    if tail > 0 {
        for (index, line) in critical.iter().skip(critical.len().saturating_sub(tail)) {
            output.push(format!(
                "  L{}: {}",
                index + 1,
                truncate_command_line(line.trim(), options.max_line_chars)
            ));
        }
    }
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
        "# prodex context saver: {} ({} -> {} lines)",
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

    if looks_like_log_stream_output(&lines) {
        return CommandOutputKind::Plain;
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

    if looks_like_noisy_success_output(&lines) {
        return CommandOutputKind::NoisySuccess;
    }

    CommandOutputKind::Plain
}

fn is_error_signal_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("error:")
        || lower.starts_with("error[")
        || lower.starts_with("error ")
        || lower.starts_with("error\t")
        || lower.starts_with("fatal:")
        || lower.starts_with("panic:")
        || lower.starts_with("npm err!")
        || lower.starts_with("failed ")
        || lower.starts_with("fail ")
        || trimmed.starts_with("E   ")
        || lower.starts_with("thread '") && lower.contains("' panicked at")
        || is_rust_panic_line(line)
        || is_typescript_diagnostic_line(line)
        || is_exception_signal_line(line)
        || is_log_level_signal_line(line)
    {
        return true;
    }

    contains_jsonish_error_key(trimmed)
        || lower.contains(" status=error")
        || lower.starts_with("status=error")
        || lower.contains(" level=error")
        || lower.starts_with("level=error")
}

fn contains_jsonish_error_key(line: &str) -> bool {
    line.contains("\"error\"")
        || line.contains("'error'")
        || line.contains("\\\"error\\\"")
        || line.contains("\"type\":\"error\"")
        || line.contains("\"type\": \"error\"")
        || line.contains("\\\"type\\\":\\\"error\\\"")
        || line.contains("\\\"type\\\": \\\"error\\\"")
}

fn count_file_location_signals(line: &str) -> usize {
    let token_locations = line
        .split_whitespace()
        .filter(|token| token_contains_file_location(token))
        .count();
    let python_location = usize::from(contains_python_file_location(line));
    let paren_location = usize::from(contains_paren_file_location(line));
    token_locations + python_location + paren_location
}

fn token_contains_file_location(token: &str) -> bool {
    let token = token.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\'' | '`' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}'
        )
    });
    let token = token.trim_end_matches([':', '.']);
    if token.contains("://") || !token.chars().any(|ch| ch.is_ascii_digit()) {
        return false;
    }

    let mut segments = token.rsplitn(3, ':');
    let tail = segments.next().unwrap_or_default().trim_end_matches('.');
    let middle = segments.next().unwrap_or_default();
    let path = segments.next().unwrap_or_default();

    if !tail.chars().all(|ch| ch.is_ascii_digit()) || tail.is_empty() {
        return false;
    }

    if middle.chars().all(|ch| ch.is_ascii_digit()) && !middle.is_empty() {
        return looks_like_location_path(path);
    }

    looks_like_location_path(middle)
}

fn contains_python_file_location(line: &str) -> bool {
    let trimmed = line.trim_start();
    let (quote, rest) = if let Some(rest) = trimmed.strip_prefix("File \"") {
        ('"', rest)
    } else if let Some(rest) = trimmed.strip_prefix("File '") {
        ('\'', rest)
    } else {
        return false;
    };
    let Some((path, after_path)) = rest.split_once(quote) else {
        return false;
    };
    if !looks_like_location_path(path) || !after_path.contains(", line ") {
        return false;
    }
    let Some((_, after_line)) = after_path.split_once(", line ") else {
        return false;
    };
    after_line
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit())
}

fn contains_paren_file_location(line: &str) -> bool {
    line.split_whitespace()
        .any(|token| token_contains_paren_file_location(token))
}

fn token_contains_paren_file_location(token: &str) -> bool {
    let token = token.trim_matches(|ch: char| {
        matches!(ch, '"' | '\'' | '`' | ',' | ';' | '[' | ']' | '{' | '}')
    });
    let Some((path, rest)) = token.split_once('(') else {
        return false;
    };
    let Some((location, _)) = rest.split_once(')') else {
        return false;
    };
    let Some((line, column)) = location.split_once(',') else {
        return false;
    };
    looks_like_location_path(path)
        && !line.trim().is_empty()
        && line.trim().chars().all(|ch| ch.is_ascii_digit())
        && !column.trim().is_empty()
        && column.trim().chars().all(|ch| ch.is_ascii_digit())
}

fn looks_like_location_path(path: &str) -> bool {
    let path = path.trim_matches(|ch: char| matches!(ch, '<' | '>' | '-' | ':' | ' '));
    if path.is_empty() {
        return false;
    }
    path.contains('/')
        || path.contains('\\')
        || path.rsplit('/').next().is_some_and(|name| {
            name.rsplit_once('.').is_some_and(|(_, ext)| {
                !ext.is_empty()
                    && ext.len() <= 12
                    && ext
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
            })
        })
}

fn is_diff_hunk_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("@@ ") && trimmed[3..].contains("@@")
}

fn is_test_failure_signal_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    rust_failed_test_name(trimmed).is_some()
        || rust_failure_separator_name(trimmed).is_some()
        || generic_failed_test_name(trimmed).is_some()
        || is_rust_failure_summary_line(trimmed)
        || trimmed.starts_with("test result: FAILED")
        || trimmed.starts_with("failures:")
        || trimmed.contains(" ... FAILED")
}

fn is_stack_signal_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    is_rust_backtrace_start(trimmed)
        || trimmed.starts_with("Traceback (most recent call last):")
        || trimmed.starts_with("Stack trace:")
        || trimmed.starts_with("stack trace:")
        || trimmed.starts_with("Backtrace:")
        || trimmed.starts_with("Caused by:")
}

fn is_rust_diagnostic_signal_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    rust_diagnostic_severity(trimmed).is_some()
        || trimmed.starts_with("--> ")
        || trimmed.starts_with("::: ")
        || trimmed.starts_with("= note:")
        || trimmed.starts_with("= help:")
        || trimmed.starts_with("help:")
        || trimmed.starts_with("note:")
        || trimmed.starts_with("warning:")
        || trimmed.starts_with("warning[")
        || trimmed.contains("clippy::")
}

fn is_lockfile_or_vendor_path(path: &Path) -> bool {
    let rendered = path.display().to_string();
    let normalized = rendered.replace('\\', "/").to_ascii_lowercase();
    normalized.ends_with("cargo.lock")
        || normalized.ends_with("package-lock.json")
        || normalized.ends_with("pnpm-lock.yaml")
        || normalized.ends_with("yarn.lock")
        || normalized.contains("/node_modules/")
        || normalized.contains("/vendor/")
        || normalized.contains("/target/")
}

fn looks_like_base64_blob(line: &str) -> bool {
    let base64_chars = line
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '=' | '-' | '_'))
        .count();
    base64_chars.saturating_mul(100) >= line.len().saturating_mul(95)
        && line.chars().any(|ch| ch.is_ascii_digit())
        && line.chars().any(|ch| ch.is_ascii_uppercase())
        && line.chars().any(|ch| ch.is_ascii_lowercase())
}

fn looks_like_minified_js_json(line: &str) -> bool {
    let punctuation = line
        .chars()
        .filter(|ch| matches!(ch, '{' | '}' | '[' | ']' | ':' | ',' | ';'))
        .count();
    let spaces = line.chars().filter(|ch| ch.is_whitespace()).count();
    punctuation >= 32 && spaces.saturating_mul(80) < line.len()
}

fn repeated_path_flood(lines: &[&str]) -> Option<(usize, usize)> {
    let mut first_path_line = None;
    let mut path_lines = 0usize;
    let mut non_empty = 0usize;
    for (index, line) in lines.iter().enumerate() {
        if !line.trim().is_empty() {
            non_empty += 1;
        }
        if parse_file_list_entry_line(line).is_some() || parse_search_match_line(line).is_some() {
            path_lines += 1;
            first_path_line.get_or_insert(index + 1);
        }
    }

    if path_lines >= 200 && path_lines.saturating_mul(100) >= non_empty.saturating_mul(80) {
        first_path_line.map(|line| (line, path_lines))
    } else {
        None
    }
}

fn add_context_blob_noise_supplemental_findings(
    path: Option<&Path>,
    input: &str,
    lines: &[&str],
    findings: &mut Vec<ContextBlobNoiseFinding>,
) {
    if let Some(finding) = detect_binaryish_text_noise_supplement(input) {
        push_context_blob_noise_finding(findings, finding);
    }
    if let Some(finding) = detect_base64ish_blob_noise_supplement(lines) {
        push_context_blob_noise_finding(findings, finding);
    }
    if let Some(finding) = detect_minified_js_json_noise_supplement(input, lines) {
        push_context_blob_noise_finding(findings, finding);
    }
    if let Some(finding) = detect_lockfile_or_vendor_noise_supplement(path, input, lines) {
        push_context_blob_noise_finding(findings, finding);
    }
    if let Some(finding) = detect_repeated_path_flood_noise_supplement(input, lines) {
        push_context_blob_noise_finding(findings, finding);
    }
}

fn push_context_blob_noise_finding(
    findings: &mut Vec<ContextBlobNoiseFinding>,
    finding: ContextBlobNoiseFinding,
) {
    if !findings
        .iter()
        .any(|existing| existing.kind == finding.kind)
    {
        findings.push(finding);
    }
}

fn detect_binaryish_text_noise_supplement(input: &str) -> Option<ContextBlobNoiseFinding> {
    let total_chars = input.chars().count();
    if total_chars < 16 {
        return None;
    }

    let mut suspicious_chars = 0usize;
    let mut nul_chars = 0usize;
    let mut replacement_chars = 0usize;
    let mut first_line = None;
    for (line_index, line) in input.lines().enumerate() {
        for ch in line.chars() {
            let suspicious = ch == '\0'
                || ch == '\u{fffd}'
                || (ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'));
            if !suspicious {
                continue;
            }
            first_line.get_or_insert(line_index + 1);
            suspicious_chars += 1;
            if ch == '\0' {
                nul_chars += 1;
            } else if ch == '\u{fffd}' {
                replacement_chars += 1;
            }
        }
    }

    let suspicious_ratio = suspicious_chars.saturating_mul(100) / total_chars.max(1);
    if nul_chars == 0 && replacement_chars < 2 && (suspicious_chars < 4 || suspicious_ratio < 2) {
        return None;
    }

    Some(ContextBlobNoiseFinding {
        kind: ContextBlobNoiseKind::BinaryText,
        line: first_line,
        bytes: input.len(),
        score: (60 + suspicious_ratio).min(100),
        detail: format!(
            "binaryish_controls={suspicious_chars}, nul_chars={nul_chars}, replacement_chars={replacement_chars}"
        ),
    })
}

fn detect_base64ish_blob_noise_supplement(lines: &[&str]) -> Option<ContextBlobNoiseFinding> {
    let mut best_line = None;
    let mut best_bytes = 0usize;
    let mut best_score = 0usize;
    let mut block_start = 0usize;
    let mut block_lines = 0usize;
    let mut block_bytes = 0usize;
    let mut block_score = 0usize;

    for (index, line) in lines.iter().enumerate() {
        if let Some((bytes, score)) = longest_base64ish_span_supplement(line, 160, 16)
            && bytes > best_bytes
        {
            best_line = Some(index + 1);
            best_bytes = bytes;
            best_score = score;
        }

        let trimmed = line.trim();
        if let Some(score) = base64ish_candidate_score_supplement(trimmed, 56, 12) {
            if block_lines == 0 {
                block_start = index + 1;
            }
            block_lines += 1;
            block_bytes += trimmed.len();
            block_score = block_score.max(score);
        } else {
            if block_lines >= 4 && block_bytes >= 240 && block_bytes > best_bytes {
                best_line = Some(block_start);
                best_bytes = block_bytes;
                best_score = block_score;
            }
            block_lines = 0;
            block_bytes = 0;
            block_score = 0;
        }
    }

    if block_lines >= 4 && block_bytes >= 240 && block_bytes > best_bytes {
        best_line = Some(block_start);
        best_bytes = block_bytes;
        best_score = block_score;
    }

    (best_bytes > 0).then(|| ContextBlobNoiseFinding {
        kind: ContextBlobNoiseKind::Base64Blob,
        line: best_line,
        bytes: best_bytes,
        score: best_score,
        detail: format!("base64ish_bytes={best_bytes}"),
    })
}

fn longest_base64ish_span_supplement(
    line: &str,
    min_bytes: usize,
    min_unique_chars: usize,
) -> Option<(usize, usize)> {
    let mut best = None;
    let mut start = None;

    for (index, ch) in line.char_indices() {
        if is_base64ish_char_supplement(ch) {
            start.get_or_insert(index);
            continue;
        }
        if let Some(span_start) = start.take() {
            let candidate = &line[span_start..index];
            if let Some(score) =
                base64ish_candidate_score_supplement(candidate, min_bytes, min_unique_chars)
            {
                best = Some(match best {
                    Some((bytes, best_score)) if bytes >= candidate.len() => (bytes, best_score),
                    _ => (candidate.len(), score),
                });
            }
        }
    }

    if let Some(span_start) = start {
        let candidate = &line[span_start..];
        if let Some(score) =
            base64ish_candidate_score_supplement(candidate, min_bytes, min_unique_chars)
        {
            best = Some(match best {
                Some((bytes, best_score)) if bytes >= candidate.len() => (bytes, best_score),
                _ => (candidate.len(), score),
            });
        }
    }

    best
}

fn base64ish_candidate_score_supplement(
    candidate: &str,
    min_bytes: usize,
    min_unique_chars: usize,
) -> Option<usize> {
    let candidate = candidate.trim_matches(|ch| matches!(ch, '"' | '\'' | '`' | ',' | ';'));
    if candidate.len() < min_bytes || !candidate.is_ascii() {
        return None;
    }

    let mut base64_chars = 0usize;
    let mut upper = 0usize;
    let mut lower = 0usize;
    let mut digit = 0usize;
    let mut symbol = 0usize;
    let mut seen = [false; 128];
    for byte in candidate.bytes() {
        let ch = byte as char;
        if !is_base64ish_char_supplement(ch) {
            continue;
        }
        base64_chars += 1;
        seen[byte as usize] = true;
        if ch.is_ascii_uppercase() {
            upper += 1;
        } else if ch.is_ascii_lowercase() {
            lower += 1;
        } else if ch.is_ascii_digit() {
            digit += 1;
        } else {
            symbol += 1;
        }
    }

    let unique_chars = seen.into_iter().filter(|seen| *seen).count();
    let ratio = base64_chars.saturating_mul(100) / candidate.len().max(1);
    let classes = [upper, lower, digit, symbol]
        .into_iter()
        .filter(|count| *count > 0)
        .count();
    if ratio < 96 || unique_chars < min_unique_chars || classes < 2 {
        return None;
    }

    Some((70 + ratio.saturating_sub(96).saturating_mul(8)).min(100))
}

fn is_base64ish_char_supplement(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '_' | '-' | '=')
}

fn detect_minified_js_json_noise_supplement(
    input: &str,
    lines: &[&str],
) -> Option<ContextBlobNoiseFinding> {
    let trimmed = input.trim();
    if trimmed.len() < 512 {
        return None;
    }

    let non_empty_lines = lines.iter().filter(|line| !line.trim().is_empty()).count();
    let max_line_bytes = lines
        .iter()
        .map(|line| line.trim().len())
        .max()
        .unwrap_or(0);
    if non_empty_lines > 4 && max_line_bytes < 512 {
        return None;
    }

    let chars = trimmed.chars().count();
    let whitespace = trimmed.chars().filter(|ch| ch.is_whitespace()).count();
    if whitespace.saturating_mul(100) > chars.max(1).saturating_mul(5) {
        return None;
    }

    let punctuation = trimmed
        .chars()
        .filter(|ch| matches!(ch, '{' | '}' | '[' | ']' | '(' | ')' | ':' | ',' | ';'))
        .count();
    if punctuation.saturating_mul(100) < chars.max(1).saturating_mul(8) {
        return None;
    }

    let (detail, score) = if looks_like_minified_json_supplement(trimmed) {
        ("minified_json", 92)
    } else if looks_like_minified_javascript_supplement(trimmed) {
        ("minified_javascript", 88)
    } else {
        return None;
    };

    Some(ContextBlobNoiseFinding {
        kind: ContextBlobNoiseKind::MinifiedJsJson,
        line: Some(1),
        bytes: trimmed.len(),
        score,
        detail: format!("{detail}, max_line_bytes={max_line_bytes}"),
    })
}

fn looks_like_minified_json_supplement(input: &str) -> bool {
    (input.starts_with('{') && input.ends_with('}')
        || input.starts_with('[') && input.ends_with(']'))
        && input.matches(':').count() >= 8
        && input.matches(',').count() >= 8
        && input.matches('"').count() >= 16
}

fn looks_like_minified_javascript_supplement(input: &str) -> bool {
    let function_markers = count_substring_supplement(input, "function(")
        + count_substring_supplement(input, "=>")
        + count_substring_supplement(input, "module.exports")
        + count_substring_supplement(input, "exports.");
    let semicolons = input.matches(';').count();
    let braces = input.matches('{').count() + input.matches('}').count();
    let parens = input.matches('(').count() + input.matches(')').count();

    function_markers >= 2 && (semicolons >= 8 || braces + parens >= 32)
}

fn count_substring_supplement(input: &str, needle: &str) -> usize {
    input.match_indices(needle).count()
}

fn detect_lockfile_or_vendor_noise_supplement(
    path: Option<&Path>,
    input: &str,
    lines: &[&str],
) -> Option<ContextBlobNoiseFinding> {
    if let Some(path) = path
        && let Some(detail) = context_lockfile_or_vendor_path_detail_supplement(path)
    {
        return Some(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: None,
            bytes: input.len(),
            score: 100,
            detail,
        });
    }

    let cargo_packages = lines
        .iter()
        .filter(|line| line.trim() == "[[package]]")
        .count();
    let cargo_checksums = lines
        .iter()
        .filter(|line| line.trim_start().starts_with("checksum = "))
        .count();
    if cargo_packages >= 4 && cargo_checksums >= 2 {
        return Some(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: Some(1),
            bytes: input.len(),
            score: 95,
            detail: format!("cargo_lock_packages={cargo_packages}"),
        });
    }

    let lower = input.to_ascii_lowercase();
    if lower.contains("\"lockfileversion\"")
        && (lower.contains("\"packages\"") || lower.contains("\"dependencies\""))
    {
        return Some(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: Some(1),
            bytes: input.len(),
            score: 95,
            detail: "npm_lockfile_json".to_string(),
        });
    }

    if lower.contains("# yarn lockfile")
        || lower.contains("pnpm-lock.yaml")
        || lower.contains("lockfileversion:")
            && (lower.contains("importers:") || lower.contains("packages:"))
    {
        return Some(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: Some(1),
            bytes: input.len(),
            score: 90,
            detail: "package_manager_lockfile".to_string(),
        });
    }

    let non_empty_lines = lines.iter().filter(|line| !line.trim().is_empty()).count();
    let mut vendor_path_lines = 0usize;
    let mut first_vendor_line = None;
    for (index, line) in lines.iter().enumerate() {
        if context_line_has_vendor_path_supplement(line) {
            vendor_path_lines += 1;
            first_vendor_line.get_or_insert(index + 1);
        }
    }
    if vendor_path_lines >= 8
        && vendor_path_lines.saturating_mul(100) >= non_empty_lines.max(1) * 40
    {
        return Some(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: first_vendor_line,
            bytes: input.len(),
            score: 85,
            detail: format!("vendor_path_lines={vendor_path_lines}"),
        });
    }

    None
}

fn context_lockfile_or_vendor_path_detail_supplement(path: &Path) -> Option<String> {
    let normalized = path.display().to_string().replace('\\', "/");
    let lower = normalized.to_ascii_lowercase();
    let file_name = lower.rsplit('/').next().unwrap_or(lower.as_str());
    if matches!(
        file_name,
        "cargo.lock"
            | "package-lock.json"
            | "npm-shrinkwrap.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "bun.lock"
            | "bun.lockb"
            | "poetry.lock"
            | "pipfile.lock"
            | "gemfile.lock"
            | "composer.lock"
            | "go.sum"
    ) {
        return Some(format!("lockfile_path={normalized}"));
    }

    [
        "/node_modules/",
        "/vendor/",
        "/third_party/",
        "/.cargo/registry/",
        "/.pnpm/",
        "/.yarn/cache/",
    ]
    .into_iter()
    .find(|marker| {
        lower.contains(marker) || lower.trim_start_matches("./").starts_with(&marker[1..])
    })
    .map(|_| format!("vendor_path={normalized}"))
}

fn context_line_has_vendor_path_supplement(line: &str) -> bool {
    let lower = line.replace('\\', "/").to_ascii_lowercase();
    [
        "/node_modules/",
        "node_modules/",
        "/vendor/",
        "vendor/",
        "/third_party/",
        "third_party/",
        "/.cargo/registry/",
        ".cargo/registry/",
        "/.pnpm/",
        ".pnpm/",
        "/.yarn/cache/",
        ".yarn/cache/",
    ]
    .into_iter()
    .any(|marker| lower.contains(marker))
}

fn detect_repeated_path_flood_noise_supplement(
    input: &str,
    lines: &[&str],
) -> Option<ContextBlobNoiseFinding> {
    let mut path_counts = BTreeMap::<String, usize>::new();
    let mut prefix_counts = BTreeMap::<String, usize>::new();
    let mut first_path_line = BTreeMap::<String, usize>::new();
    let mut path_lines = 0usize;

    for (index, line) in lines.iter().enumerate() {
        let Some(path) = context_noise_extract_path_key_supplement(line) else {
            continue;
        };
        path_lines += 1;
        first_path_line.entry(path.clone()).or_insert(index + 1);
        *path_counts.entry(path.clone()).or_default() += 1;
        *prefix_counts
            .entry(context_noise_path_prefix_supplement(&path))
            .or_default() += 1;
    }

    if path_lines < 16 {
        return None;
    }

    let (top_path, top_path_count) = path_counts
        .iter()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))?;
    let top_prefix_count = prefix_counts.values().copied().max().unwrap_or(0);
    let unique_paths = path_counts.len();
    let exact_flood = *top_path_count >= 8 && top_path_count.saturating_mul(100) >= path_lines * 35;
    let low_variety_flood = path_lines >= 24 && unique_paths.saturating_mul(100) <= path_lines * 20;
    let prefix_flood =
        top_prefix_count >= 24 && top_prefix_count.saturating_mul(100) >= path_lines * 70;

    if !exact_flood && !low_variety_flood && !prefix_flood {
        return None;
    }

    Some(ContextBlobNoiseFinding {
        kind: ContextBlobNoiseKind::RepeatedPathFlood,
        line: first_path_line.get(top_path.as_str()).copied(),
        bytes: input.len(),
        score: if exact_flood { 92 } else { 84 },
        detail: format!(
            "path_lines={path_lines}, unique_paths={unique_paths}, top_path_count={top_path_count}, top_path={top_path}"
        ),
    })
}

fn context_noise_extract_path_key_supplement(line: &str) -> Option<String> {
    if let Some(search_match) =
        parse_search_match_line(line).or_else(|| parse_rg_json_match_line(line))
    {
        return context_noise_normalize_path_token_supplement(&search_match.path);
    }

    if let Some(path) = parse_file_list_entry_line(line) {
        return context_noise_normalize_path_token_supplement(&path);
    }

    line.split_whitespace()
        .filter_map(context_noise_normalize_path_token_supplement)
        .next()
}

fn context_noise_normalize_path_token_supplement(token: &str) -> Option<String> {
    let token = token.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\'' | '`' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
        )
    });
    if token.contains("://") {
        return None;
    }

    let normalized = token.replace('\\', "/");
    let stripped = context_noise_strip_path_location_suffix_supplement(&normalized);
    let stripped = stripped
        .trim_start_matches("a/")
        .trim_start_matches("b/")
        .trim_start_matches("./")
        .trim_matches('/');
    if stripped.len() < 3 || !stripped.contains('/') || stripped.contains(' ') {
        return None;
    }

    Some(stripped.to_string())
}

fn context_noise_strip_path_location_suffix_supplement(path: &str) -> &str {
    let mut value = path;
    for _ in 0..2 {
        let Some((head, tail)) = value.rsplit_once(':') else {
            break;
        };
        if tail.is_empty() || !tail.chars().all(|ch| ch.is_ascii_digit()) {
            break;
        }
        value = head;
    }
    value
}

fn context_noise_path_prefix_supplement(path: &str) -> String {
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .take(3)
        .collect::<Vec<_>>();
    if segments.len() <= 1 {
        path.to_string()
    } else {
        segments.join("/")
    }
}

fn normalize_command_output(input: &str) -> String {
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

fn command_lines(input: &str) -> Vec<&str> {
    input
        .trim_end_matches('\n')
        .split('\n')
        .filter(|line| !(line.is_empty() && input.is_empty()))
        .collect()
}

fn count_text_lines(input: &str) -> usize {
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
}

#[derive(Default)]
struct RustDiagnosticSummary {
    errors: usize,
    warnings: usize,
    panics: usize,
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
    }

    fn record_failed_test(&mut self, test_name: &str) {
        push_unique_line(&mut self.failed_tests, test_name.trim());
    }

    fn record_location(&mut self, line: &str) {
        push_unique_line(&mut self.locations, line.trim());
    }

    fn record_exit_status(&mut self, line: &str) {
        push_unique_line(&mut self.exit_statuses, line.trim());
    }

    fn record_block_signals(&mut self, block: &RustCriticalBlock) {
        for line in &block.lines {
            if is_rust_location_line(line) {
                self.record_location(line);
            }
            if is_rust_panic_line(line) {
                self.panics += 1;
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
enum RustDiagnosticSeverity {
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
    diagnostic_headers: Vec<String>,
    locations: Vec<String>,
    failed_tests: Vec<String>,
    exit_statuses: Vec<String>,
}

impl CommandDiagnosticSummary {
    fn is_empty(&self) -> bool {
        self.errors == 0
            && self.stack_markers == 0
            && self.diagnostic_headers.is_empty()
            && self.locations.is_empty()
            && self.failed_tests.is_empty()
            && self.exit_statuses.is_empty()
    }

    fn record_line(&mut self, line: &str) {
        if is_error_signal_line(line) || is_typescript_diagnostic_line(line) {
            self.errors += 1;
            push_unique_truncated_line(&mut self.diagnostic_headers, line, 240);
        }
        if let Some(test_name) = generic_failed_test_name(line) {
            push_unique_line(&mut self.failed_tests, test_name);
        }
        if count_file_location_signals(line) > 0 {
            push_unique_truncated_line(&mut self.locations, line, 240);
        }
        if is_rust_exit_status_line(line) {
            push_unique_truncated_line(&mut self.exit_statuses, line, 240);
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
    };
    for line in section {
        if line.starts_with("@@ ") {
            summary.hunks += 1;
        } else if line.starts_with('+') && !line.starts_with("+++") {
            summary.added += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            summary.removed += 1;
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
struct SearchMatch {
    path: String,
    line_number: Option<usize>,
    text: String,
}

fn parse_search_match_line(line: &str) -> Option<SearchMatch> {
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

fn parse_rg_json_match_line(line: &str) -> Option<SearchMatch> {
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

fn parse_file_list_entry_line(line: &str) -> Option<String> {
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

fn is_log_level_signal_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    contains_json_log_level(&lower, "error")
        || contains_json_log_level(&lower, "fatal")
        || contains_json_log_level(&lower, "warn")
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

impl CommandOutputKind {
    fn label(self) -> &'static str {
        match self {
            CommandOutputKind::Auto => "auto",
            CommandOutputKind::GitStatus => "git status",
            CommandOutputKind::GitDiff => "git diff",
            CommandOutputKind::RustDiagnostics => "rust diagnostics",
            CommandOutputKind::Diagnostics => "diagnostics",
            CommandOutputKind::GitLog => "git log",
            CommandOutputKind::Search => "search",
            CommandOutputKind::FileList => "file list",
            CommandOutputKind::NoisySuccess => "noisy success",
            CommandOutputKind::Plain => "plain",
        }
    }
}

pub fn compress_context_text(input: &str) -> String {
    let mut output = Vec::new();
    let mut paragraph = Vec::new();
    let mut in_fence = false;
    let mut previous_blank = false;

    for line in input.lines() {
        let trimmed = line.trim();
        let fence = trimmed.starts_with("```") || trimmed.starts_with("~~~");
        if fence {
            flush_context_paragraph(&mut paragraph, &mut output);
            output.push(line.to_string());
            in_fence = !in_fence;
            previous_blank = false;
            continue;
        }

        if in_fence || protected_context_line(line) {
            flush_context_paragraph(&mut paragraph, &mut output);
            output.push(line.to_string());
            previous_blank = false;
            continue;
        }

        if trimmed.is_empty() {
            flush_context_paragraph(&mut paragraph, &mut output);
            if !previous_blank && !output.is_empty() {
                output.push(String::new());
            }
            previous_blank = true;
            continue;
        }

        paragraph.push(trimmed.to_string());
        previous_blank = false;
    }

    flush_context_paragraph(&mut paragraph, &mut output);
    while output.last().is_some_and(|line| line.is_empty()) {
        output.pop();
    }
    if output.is_empty() {
        String::new()
    } else {
        format!("{}\n", output.join("\n"))
    }
}

fn flush_context_paragraph(paragraph: &mut Vec<String>, output: &mut Vec<String>) {
    if paragraph.is_empty() {
        return;
    }
    let joined = paragraph.join(" ");
    output.push(compact_context_prose(&joined));
    paragraph.clear();
}

fn compact_context_prose(input: &str) -> String {
    let normalized = input.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.contains('`') || normalized.contains("://") {
        return normalized;
    }

    let mut text = format!(" {normalized} ");
    for (from, to) in [
        (" in order to ", " to "),
        (" due to the fact that ", " because "),
        (" at this point in time ", " now "),
        (" make sure to ", " ensure "),
        (" it is important to ", " "),
        (" please note that ", " "),
        (" you should ", " should "),
        (" you must ", " must "),
    ] {
        text = text.replace(from, to);
    }

    text.split_whitespace()
        .filter(|word| !is_context_filler_word(word))
        .collect::<Vec<_>>()
        .join(" ")
}

fn protected_context_line(line: &str) -> bool {
    let trimmed = line.trim();
    let indented = line.starts_with("    ") || line.starts_with('\t');
    trimmed.starts_with('#')
        || trimmed.starts_with('|')
        || trimmed.starts_with('>')
        || trimmed.starts_with('$')
        || trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.contains('`')
        || trimmed.contains("://")
        || indented
}

fn is_context_filler_word(word: &str) -> bool {
    let normalized = word
        .trim_matches(|ch: char| !ch.is_alphanumeric())
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "very" | "really" | "actually" | "basically" | "simply" | "please" | "just"
    )
}

fn estimate_context_tokens(chars: usize, words: usize) -> usize {
    chars.div_ceil(4).max((words * 4).div_ceil(3))
}

fn is_auditable_context_file(path: &Path) -> bool {
    !is_context_backup(path)
        && path.is_file()
        && (is_compressible_context_file(path)
            || matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("toml" | "json" | "yaml" | "yml")
            ))
}

fn is_compressible_context_file(path: &Path) -> bool {
    !is_context_backup(path)
        && path.is_file()
        && matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("md" | "markdown" | "txt")
        )
}

fn is_context_backup(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".original.md"))
}

fn context_backup_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("context");
    parent.join(format!("{stem}.original.md"))
}

fn format_count<T: std::fmt::Display>(value: T) -> String {
    value.to_string()
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
