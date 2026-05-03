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
    Search,
    FileList,
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
        CommandOutputKind::Search => compact_search_output(&normalized, options),
        CommandOutputKind::FileList => compact_file_list_output(&normalized, options),
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

fn compact_search_output(input: &str, options: &CommandOutputCompactOptions) -> String {
    let lines = command_lines(input);
    let mut files: BTreeMap<String, Vec<SearchMatch>> = BTreeMap::new();
    let mut other = Vec::new();

    for line in lines {
        if let Some(search_match) = parse_search_match_line(line) {
            files
                .entry(search_match.path.clone())
                .or_default()
                .push(search_match);
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
        if looks_like_file_list_line(line) {
            entries.push(line.trim().to_string());
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
    if lines.iter().any(|line| line.starts_with("diff --git "))
        || lines
            .iter()
            .filter(|line| line.starts_with("@@ "))
            .take(2)
            .count()
            >= 1
    {
        return CommandOutputKind::GitDiff;
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
        .filter(|line| parse_search_match_line(line).is_some())
        .count();
    if search_matches >= 2 && search_matches.saturating_mul(2) >= non_empty {
        return CommandOutputKind::Search;
    }

    let file_list_lines = lines
        .iter()
        .filter(|line| looks_like_file_list_line(line))
        .count();
    if file_list_lines >= 4 && file_list_lines.saturating_mul(2) >= non_empty {
        return CommandOutputKind::FileList;
    }

    CommandOutputKind::Plain
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

fn split_git_diff_sections<'a>(lines: &'a [&'a str]) -> Vec<Vec<&'a str>> {
    let mut sections = Vec::new();
    let mut current = Vec::new();
    for line in lines {
        if line.starts_with("diff --git ") && !current.is_empty() {
            sections.push(current);
            current = Vec::new();
        }
        if line.starts_with("diff --git ") || !current.is_empty() {
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

fn looks_like_search_path(path: &str) -> bool {
    path.contains('/') || path.contains('\\') || path.contains('.')
}

fn looks_like_file_list_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("[...")
        || trimmed.contains(" directories, ")
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

impl CommandOutputKind {
    fn label(self) -> &'static str {
        match self {
            CommandOutputKind::Auto => "auto",
            CommandOutputKind::GitStatus => "git status",
            CommandOutputKind::GitDiff => "git diff",
            CommandOutputKind::Search => "search",
            CommandOutputKind::FileList => "file list",
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
#[path = "../../../tests/unit/crates/prodex-context/src/lib.rs"]
mod tests;
