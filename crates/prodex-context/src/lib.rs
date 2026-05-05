use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
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
const STATIC_DUPLICATE_MIN_CHARS: usize = 80;
const STATIC_DUPLICATE_MIN_WORDS: usize = 10;
const STATIC_DUPLICATE_PREVIEW_CHARS: usize = 160;

#[derive(Debug, Clone)]
struct ContextStaticDuplicateCandidate {
    key: String,
    preview: String,
    occurrence: ContextStaticDuplicateOccurrence,
    words: usize,
    normalized_chars: usize,
    estimated_tokens: usize,
}

#[derive(Debug, Clone)]
struct CommandPathAlias {
    alias: String,
    prefix: String,
}

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
    pub static_duplicates: ContextStaticDuplicateReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextStaticDuplicateOccurrence {
    pub path: PathBuf,
    pub relative_path: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextStaticDuplicateSnippet {
    pub preview: String,
    pub occurrence_count: usize,
    pub occurrences: Vec<ContextStaticDuplicateOccurrence>,
    pub words: usize,
    pub normalized_chars: usize,
    pub estimated_tokens_per_occurrence: usize,
    pub estimated_duplicate_tokens: usize,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextStaticDuplicateReport {
    pub root: PathBuf,
    pub snippets: Vec<ContextStaticDuplicateSnippet>,
    pub total_duplicate_snippets: usize,
    pub hidden_duplicate_snippets: usize,
    pub total_duplicate_occurrences: usize,
    pub estimated_duplicate_tokens: usize,
    pub suggestion: String,
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

    fn add_assign(&mut self, other: Self) {
        self.errors = self.errors.saturating_add(other.errors);
        self.file_locations = self.file_locations.saturating_add(other.file_locations);
        self.diff_hunks = self.diff_hunks.saturating_add(other.diff_hunks);
        self.test_failures = self.test_failures.saturating_add(other.test_failures);
        self.exit_codes = self.exit_codes.saturating_add(other.exit_codes);
        self.stack_markers = self.stack_markers.saturating_add(other.stack_markers);
        self.rust_diagnostics = self.rust_diagnostics.saturating_add(other.rust_diagnostics);
    }

    fn subtract_assign(&mut self, other: Self) {
        self.errors = self.errors.saturating_sub(other.errors);
        self.file_locations = self.file_locations.saturating_sub(other.file_locations);
        self.diff_hunks = self.diff_hunks.saturating_sub(other.diff_hunks);
        self.test_failures = self.test_failures.saturating_sub(other.test_failures);
        self.exit_codes = self.exit_codes.saturating_sub(other.exit_codes);
        self.stack_markers = self.stack_markers.saturating_sub(other.stack_markers);
        self.rust_diagnostics = self.rust_diagnostics.saturating_sub(other.rust_diagnostics);
    }

    fn overlaps(self, other: Self) -> bool {
        self.errors > 0 && other.errors > 0
            || self.file_locations > 0 && other.file_locations > 0
            || self.diff_hunks > 0 && other.diff_hunks > 0
            || self.test_failures > 0 && other.test_failures > 0
            || self.exit_codes > 0 && other.exit_codes > 0
            || self.stack_markers > 0 && other.stack_markers > 0
            || self.rust_diagnostics > 0 && other.rust_diagnostics > 0
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CriticalSignalLineRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CriticalSignalLineRangeOptions {
    pub context_lines: usize,
    pub max_ranges: usize,
    pub max_range_lines: usize,
}

impl Default for CriticalSignalLineRangeOptions {
    fn default() -> Self {
        Self {
            context_lines: 1,
            max_ranges: 32,
            max_range_lines: 6,
        }
    }
}

pub fn count_critical_signals(input: &str) -> CriticalSignalCounts {
    let normalized = normalize_command_output(input);
    let mut counts = CriticalSignalCounts::default();

    for line in command_lines(&normalized) {
        counts.add_assign(critical_signal_counts_for_line(line));
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

pub fn critical_signal_lost_line_ranges(before: &str, after: &str) -> Vec<CriticalSignalLineRange> {
    critical_signal_lost_line_ranges_with_options(
        before,
        after,
        CriticalSignalLineRangeOptions::default(),
    )
}

pub fn critical_signal_lost_line_ranges_with_options(
    before: &str,
    after: &str,
    options: CriticalSignalLineRangeOptions,
) -> Vec<CriticalSignalLineRange> {
    let check = critical_signal_self_check(before, after);
    if check.passed() || options.max_ranges == 0 {
        return Vec::new();
    }

    let before = normalize_command_output(before);
    let after = normalize_command_output(after);
    let before_lines = command_lines(&before);
    let mut after_available = critical_signal_line_multiset(&after);
    let mut remaining_loss = check.lost;
    let mut ranges = Vec::<CriticalSignalLineRange>::new();

    for (line_index, line) in before_lines.iter().enumerate() {
        if remaining_loss.is_empty() {
            break;
        }

        let counts = critical_signal_counts_for_line(line);
        if counts.is_empty() {
            continue;
        }

        let key = critical_signal_line_key(line);
        if let Some(available) = after_available.get_mut(&key)
            && *available > 0
        {
            *available -= 1;
            continue;
        }

        if !counts.overlaps(remaining_loss) {
            continue;
        }

        ranges.push(critical_signal_range_around_line(
            line_index,
            before_lines.len(),
            options.context_lines,
            options.max_range_lines,
        ));
        remaining_loss.subtract_assign(counts);
    }

    merge_critical_signal_ranges(ranges, options.max_ranges)
}

fn critical_signal_counts_for_line(line: &str) -> CriticalSignalCounts {
    let mut counts = CriticalSignalCounts::default();
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
    counts
}

fn critical_signal_line_multiset(input: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::<String, usize>::new();
    for line in command_lines(input) {
        if critical_signal_counts_for_line(line).is_empty() {
            continue;
        }
        counts
            .entry(critical_signal_line_key(line))
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);
    }
    counts
}

fn critical_signal_line_key(line: &str) -> String {
    line.trim().to_string()
}

fn critical_signal_range_around_line(
    line_index: usize,
    line_count: usize,
    context_lines: usize,
    max_range_lines: usize,
) -> CriticalSignalLineRange {
    let max_range_lines = max_range_lines.max(1);
    let signal_line = line_index + 1;
    let mut start = signal_line.saturating_sub(context_lines).max(1);
    let mut end = signal_line.saturating_add(context_lines).min(line_count);

    while end.saturating_sub(start).saturating_add(1) > max_range_lines {
        if signal_line.saturating_sub(start) > end.saturating_sub(signal_line) {
            start += 1;
        } else {
            end = end.saturating_sub(1);
        }
    }

    CriticalSignalLineRange { start, end }
}

fn merge_critical_signal_ranges(
    mut ranges: Vec<CriticalSignalLineRange>,
    max_ranges: usize,
) -> Vec<CriticalSignalLineRange> {
    if ranges.is_empty() || max_ranges == 0 {
        return Vec::new();
    }

    ranges.sort_by_key(|range| (range.start, range.end));
    let mut merged = Vec::<CriticalSignalLineRange>::new();
    for range in ranges {
        if let Some(last) = merged.last_mut()
            && range.start < last.end
        {
            last.end = last.end.max(range.end);
            continue;
        }
        if merged.len() >= max_ranges {
            break;
        }
        merged.push(range);
    }
    merged
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

pub fn collect_context_static_duplicate_report(
    root: &Path,
    limit: usize,
) -> Result<ContextStaticDuplicateReport> {
    let mut paths = Vec::new();
    for entry in CONTEXT_AUDIT_ROOTS {
        collect_context_files(&root.join(entry), &mut paths)?;
    }
    paths.sort();
    paths.dedup();

    let candidates = collect_context_static_duplicate_candidates(root, &paths)?;
    Ok(build_context_static_duplicate_report(
        root.to_path_buf(),
        candidates,
        limit,
    ))
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
    let mut duplicate_candidates = Vec::new();
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
        if is_static_duplicate_context_file(&path) {
            duplicate_candidates.extend(context_static_duplicate_candidates_for_text(
                root,
                &path,
                &relative_path,
                &text,
            ));
        }
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
    let static_duplicates =
        build_context_static_duplicate_report(root.to_path_buf(), duplicate_candidates, limit);

    Ok(ContextAuditReport {
        root: root.to_path_buf(),
        files,
        total_bytes,
        total_chars,
        total_words,
        total_estimated_tokens,
        static_duplicates,
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

fn collect_context_static_duplicate_candidates(
    root: &Path,
    paths: &[PathBuf],
) -> Result<Vec<ContextStaticDuplicateCandidate>> {
    let mut candidates = Vec::new();
    for path in paths {
        if !is_static_duplicate_context_file(path) {
            continue;
        }
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(_) => continue,
        };
        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(path.as_path())
            .display()
            .to_string();
        candidates.extend(context_static_duplicate_candidates_for_text(
            root,
            path,
            &relative_path,
            &text,
        ));
    }
    Ok(candidates)
}

fn context_static_duplicate_candidates_for_text(
    _root: &Path,
    path: &Path,
    relative_path: &str,
    text: &str,
) -> Vec<ContextStaticDuplicateCandidate> {
    let mut candidates = Vec::new();
    let mut block = Vec::<String>::new();
    let mut block_start = 0usize;
    let mut in_fence = false;

    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        let fence = trimmed.starts_with("```") || trimmed.starts_with("~~~");
        if fence {
            push_context_static_duplicate_candidate(
                &mut candidates,
                path,
                relative_path,
                &mut block,
                block_start,
                line_number.saturating_sub(1),
            );
            in_fence = !in_fence;
            continue;
        }

        if in_fence || trimmed.is_empty() || is_static_context_duplicate_boundary_line(trimmed) {
            push_context_static_duplicate_candidate(
                &mut candidates,
                path,
                relative_path,
                &mut block,
                block_start,
                line_number.saturating_sub(1),
            );
            continue;
        }

        if block.is_empty() {
            block_start = line_number;
        }
        block.push(trimmed.to_string());
    }

    push_context_static_duplicate_candidate(
        &mut candidates,
        path,
        relative_path,
        &mut block,
        block_start,
        text.lines().count(),
    );
    candidates
}

fn push_context_static_duplicate_candidate(
    candidates: &mut Vec<ContextStaticDuplicateCandidate>,
    path: &Path,
    relative_path: &str,
    block: &mut Vec<String>,
    start_line: usize,
    end_line: usize,
) {
    if block.is_empty() {
        return;
    }
    let joined = block.join(" ");
    block.clear();

    let Some((key, preview, words, normalized_chars, estimated_tokens)) =
        normalize_context_static_duplicate_snippet(&joined)
    else {
        return;
    };

    candidates.push(ContextStaticDuplicateCandidate {
        key,
        preview,
        occurrence: ContextStaticDuplicateOccurrence {
            path: path.to_path_buf(),
            relative_path: relative_path.to_string(),
            start_line,
            end_line: end_line.max(start_line),
        },
        words,
        normalized_chars,
        estimated_tokens,
    });
}

fn build_context_static_duplicate_report(
    root: PathBuf,
    candidates: Vec<ContextStaticDuplicateCandidate>,
    limit: usize,
) -> ContextStaticDuplicateReport {
    let mut groups = BTreeMap::<String, Vec<ContextStaticDuplicateCandidate>>::new();
    for candidate in candidates {
        groups
            .entry(candidate.key.clone())
            .or_default()
            .push(candidate);
    }

    let mut snippets = groups
        .into_values()
        .filter(|group| group.len() > 1)
        .map(context_static_duplicate_snippet_from_group)
        .collect::<Vec<_>>();
    snippets.sort_by_key(|snippet| {
        (
            Reverse(snippet.estimated_duplicate_tokens),
            Reverse(snippet.occurrence_count),
            snippet.preview.clone(),
        )
    });

    let total_duplicate_snippets = snippets.len();
    let total_duplicate_occurrences = snippets
        .iter()
        .map(|snippet| snippet.occurrence_count)
        .sum();
    let estimated_duplicate_tokens = snippets
        .iter()
        .map(|snippet| snippet.estimated_duplicate_tokens)
        .sum();
    let shown = if limit == 0 {
        snippets.len()
    } else {
        snippets.len().min(limit)
    };
    let hidden_duplicate_snippets = snippets.len().saturating_sub(shown);
    snippets.truncate(shown);

    ContextStaticDuplicateReport {
        root,
        snippets,
        total_duplicate_snippets,
        hidden_duplicate_snippets,
        total_duplicate_occurrences,
        estimated_duplicate_tokens,
        suggestion: if total_duplicate_snippets == 0 {
            "No duplicate static context snippets found.".to_string()
        } else {
            "Review and consolidate repeated snippets manually; this report does not edit files."
                .to_string()
        },
    }
}

fn context_static_duplicate_snippet_from_group(
    mut group: Vec<ContextStaticDuplicateCandidate>,
) -> ContextStaticDuplicateSnippet {
    group.sort_by_key(|candidate| {
        (
            candidate.occurrence.relative_path.clone(),
            candidate.occurrence.start_line,
            candidate.occurrence.end_line,
        )
    });
    let first = group.first().expect("duplicate group should be non-empty");
    let estimated_tokens = first.estimated_tokens;
    let estimated_duplicate_tokens = estimated_tokens.saturating_mul(group.len().saturating_sub(1));
    let occurrence_count = group.len();
    let preview = first.preview.clone();
    let words = first.words;
    let normalized_chars = first.normalized_chars;
    let occurrences = group
        .into_iter()
        .map(|candidate| candidate.occurrence)
        .collect::<Vec<_>>();
    let primary_location = occurrences
        .first()
        .map(context_static_duplicate_location_label)
        .unwrap_or_else(|| "one canonical file".to_string());

    ContextStaticDuplicateSnippet {
        preview,
        occurrence_count,
        occurrences,
        words,
        normalized_chars,
        estimated_tokens_per_occurrence: estimated_tokens,
        estimated_duplicate_tokens,
        suggestion: format!(
            "Keep one canonical copy, for example {primary_location}, and replace other copies with a short reference if needed."
        ),
    }
}

fn normalize_context_static_duplicate_snippet(
    input: &str,
) -> Option<(String, String, usize, usize, usize)> {
    let preview = input.split_whitespace().collect::<Vec<_>>().join(" ");
    let key = normalize_context_static_duplicate_key(&preview);
    let words = key.split_whitespace().count();
    let normalized_chars = key.chars().count();
    if words < STATIC_DUPLICATE_MIN_WORDS || normalized_chars < STATIC_DUPLICATE_MIN_CHARS {
        return None;
    }
    let estimated_tokens = estimate_context_tokens(normalized_chars, words);
    Some((
        key,
        truncate_context_static_duplicate_preview(&preview),
        words,
        normalized_chars,
        estimated_tokens,
    ))
}

fn normalize_context_static_duplicate_key(input: &str) -> String {
    let mut key = String::new();
    let mut previous_space = true;
    for ch in input.chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            for lower in ch.to_lowercase() {
                key.push(lower);
            }
            previous_space = false;
        } else if !previous_space {
            key.push(' ');
            previous_space = true;
        }
    }
    key.trim().to_string()
}

fn truncate_context_static_duplicate_preview(input: &str) -> String {
    let mut preview = input
        .chars()
        .take(STATIC_DUPLICATE_PREVIEW_CHARS)
        .collect::<String>();
    if input.chars().count() > STATIC_DUPLICATE_PREVIEW_CHARS {
        preview.push_str("...");
    }
    preview
}

fn is_static_context_duplicate_boundary_line(trimmed: &str) -> bool {
    trimmed.starts_with('#')
        || trimmed.starts_with('|')
        || trimmed.starts_with('>')
        || trimmed.starts_with("```")
        || trimmed.starts_with("~~~")
}

fn context_static_duplicate_location_label(
    occurrence: &ContextStaticDuplicateOccurrence,
) -> String {
    if occurrence.start_line == occurrence.end_line {
        format!("{}:{}", occurrence.relative_path, occurrence.start_line)
    } else {
        format!(
            "{}:{}-{}",
            occurrence.relative_path, occurrence.start_line, occurrence.end_line
        )
    }
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
    lines.extend(render_context_static_duplicate_report_lines(
        &report.static_duplicates,
        total_width,
    ));
    lines.join("\n")
}

fn render_context_static_duplicate_report_lines(
    report: &ContextStaticDuplicateReport,
    total_width: usize,
) -> Vec<String> {
    let mut lines = vec![
        String::new(),
        section_header_with_width("Static Context Duplicates", total_width),
    ];
    if report.total_duplicate_snippets == 0 {
        lines.push(report.suggestion.clone());
        return lines;
    }

    lines.push(format!(
        "Found {} duplicate snippets across {} occurrences (~{} duplicate tokens).",
        format_count(report.total_duplicate_snippets),
        format_count(report.total_duplicate_occurrences),
        format_count(report.estimated_duplicate_tokens),
    ));
    lines.push(format!("Suggestion: {}", report.suggestion));
    let preview_width = total_width.saturating_sub(28).max(24);
    let detail_width = total_width.saturating_sub(13).max(24);

    for snippet in &report.snippets {
        lines.push(format!(
            "- ~{} duplicate tokens, {} copies: {}",
            format_count(snippet.estimated_duplicate_tokens),
            format_count(snippet.occurrence_count),
            fit_cell(&snippet.preview, preview_width),
        ));
        let locations = snippet
            .occurrences
            .iter()
            .map(context_static_duplicate_location_label)
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!(
            "  locations: {}",
            fit_cell(&locations, detail_width)
        ));
        lines.push(format!(
            "  suggestion: {}",
            fit_cell(&snippet.suggestion, detail_width)
        ));
    }

    if report.hidden_duplicate_snippets > 0 {
        lines.push(format!(
            "... {} more duplicate snippets hidden",
            format_count(report.hidden_duplicate_snippets),
        ));
    }
    lines
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
        CommandOutputKind::Auto => smart_truncate_command_output(&normalized, options),
        CommandOutputKind::GitStatus => compact_git_status_output(&normalized, options),
        CommandOutputKind::GitDiff => compact_git_diff_output(&normalized, options),
        CommandOutputKind::RustDiagnostics => compact_rust_diagnostic_output(&normalized, options),
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

fn normalize_intent_terms_with_prompt_expansion(terms: &[String]) -> Vec<String> {
    let mut expanded = Vec::<String>::new();
    for term in terms {
        if !term.trim().is_empty() && !term.chars().any(char::is_whitespace) {
            expanded.push(term.clone());
        }
        for extracted in extract_intent_terms_from_prompt(term) {
            expanded.push(extracted);
        }
        if expanded.len() >= MAX_EXTRACTED_INTENT_TERMS.saturating_mul(2) {
            break;
        }
    }
    normalize_intent_terms(&expanded)
        .into_iter()
        .take(MAX_EXTRACTED_INTENT_TERMS)
        .collect()
}

pub fn extract_intent_terms_from_prompt(prompt: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut seen = BTreeMap::<String, ()>::new();
    let mut token = String::new();
    let mut chars = prompt.chars().peekable();

    while let Some(ch) = chars.next() {
        if matches!(ch, '\'' | '"' | '`') {
            push_prompt_intent_candidate(&mut terms, &mut seen, &token, false);
            token.clear();

            let quote = ch;
            let mut quoted = String::new();
            for next in chars.by_ref() {
                if next == quote {
                    break;
                }
                quoted.push(next);
            }
            push_prompt_intent_candidate(&mut terms, &mut seen, &quoted, true);
        } else if is_prompt_intent_token_char(ch) {
            token.push(ch);
        } else {
            push_prompt_intent_candidate(&mut terms, &mut seen, &token, false);
            token.clear();
        }

        if terms.len() >= MAX_EXTRACTED_INTENT_TERMS {
            return terms;
        }
    }
    push_prompt_intent_candidate(&mut terms, &mut seen, &token, false);
    terms
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
    output.push("sum: git status".to_string());
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
    compact_git_diff_output_with_intent(input, options, &[])
}

fn compact_git_diff_output_with_intent(
    input: &str,
    options: &CommandOutputCompactOptions,
    intent_terms: &[String],
) -> String {
    let lines = command_lines(input);
    let sections = split_git_diff_sections(&lines);
    if sections.is_empty() {
        if let Some(output) = compact_git_diff_stat_output(&lines, options) {
            let output = finalize_compacted_command_output(
                CommandOutputKind::GitDiff,
                input,
                output,
                options,
            );
            return ensure_no_critical_signal_loss_for_intent(input, &output, options);
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

    let intent_focused = !intent_terms.is_empty();
    let structural_line_count = sections
        .iter()
        .map(|section| {
            section
                .iter()
                .filter(|line| is_git_diff_excerpt_structural_line(line, intent_focused))
                .count()
        })
        .sum::<usize>();
    let fixed_body_lines = 1usize
        .saturating_add(usize::from(intent_focused))
        .saturating_add(summaries.len())
        .saturating_add(2)
        .saturating_add(structural_line_count)
        .saturating_add(sections.len());
    let detail_budget = options
        .max_lines
        .saturating_sub(1)
        .saturating_sub(fixed_body_lines);
    let per_file_budget = detail_budget
        .checked_div(sections.len().max(1))
        .unwrap_or(0)
        .max(usize::from(detail_budget > 0));

    let mut output = Vec::new();
    output.push(format!(
        "sum: git diff files={}, +{}, -{}, hunks={}",
        summaries.len(),
        total_added,
        total_removed,
        total_hunks,
    ));
    if !intent_terms.is_empty() {
        output.push(format!(
            "int: git diff focus for {}",
            truncate_command_line(&intent_terms.join(", "), options.max_line_chars)
        ));
    }
    for summary in &summaries {
        let binary = if summary.binary { ", binary" } else { "" };
        let mut line = format!(
            "{}: +{}, -{}, {} hunks{}",
            summary.path, summary.added, summary.removed, summary.hunks, binary,
        );
        if !summary.semantic_contexts.is_empty() {
            line.push_str(&format!(
                ", ctx={}",
                truncate_command_line(&summary.semantic_contexts.join(" | "), 140)
            ));
        }
        output.push(line);
    }
    output.push(String::new());
    output.push("diff excerpts:".to_string());

    for (section, summary) in sections.iter().zip(summaries.iter()) {
        let section_score = score_git_diff_section_for_intent(section, summary, intent_terms);
        if !intent_terms.is_empty() && section_score == 0 {
            output.push(format!(
                "{}: [... omitted unchanged-to-intent diff detail ...]",
                summary.path
            ));
            continue;
        }
        let section_budget = per_file_budget;
        let selected_detail =
            select_git_diff_detail_line_indexes(section, section_budget, intent_terms);
        let mut omitted_detail = 0usize;
        for (line_index, line) in section.iter().enumerate() {
            if is_git_diff_excerpt_structural_line(line, intent_focused) {
                output.push(truncate_command_line(line, options.max_line_chars));
            } else if selected_detail.contains_key(&line_index) {
                output.push(truncate_command_line(line, options.max_line_chars));
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

    let output =
        finalize_compacted_command_output(CommandOutputKind::GitDiff, input, output, options);
    ensure_no_critical_signal_loss_for_intent(input, &output, options)
}

fn score_git_diff_section_for_intent(
    section: &[&str],
    summary: &GitDiffSummary,
    intent_terms: &[String],
) -> usize {
    if intent_terms.is_empty() {
        return 0;
    }
    let mut score = score_intent_text(&summary.path, intent_terms).saturating_mul(6);
    for context in &summary.semantic_contexts {
        score = score.saturating_add(score_intent_text(context, intent_terms).saturating_mul(4));
    }
    for line in section {
        score = score.saturating_add(score_intent_text(line, intent_terms));
    }
    score
}

fn select_git_diff_detail_line_indexes(
    section: &[&str],
    budget: usize,
    intent_terms: &[String],
) -> BTreeMap<usize, ()> {
    if budget == 0 {
        return BTreeMap::new();
    }

    let mut intent_hits = Vec::<usize>::new();
    for (index, line) in section.iter().enumerate() {
        if !is_git_diff_structural_line(line) && intent_line_matches(line, intent_terms) {
            intent_hits.push(index);
        }
    }

    let mut candidates = Vec::<(u8, usize)>::new();
    for (index, line) in section.iter().enumerate() {
        if is_git_diff_structural_line(line) {
            continue;
        }
        if intent_hits
            .iter()
            .any(|hit| hit.abs_diff(index) <= git_diff_intent_context_radius())
        {
            candidates.push((0, index));
        } else if git_diff_semantic_context_line(line).is_some() {
            candidates.push((1, index));
        } else if is_git_diff_changed_detail_line(line) {
            candidates.push((2, index));
        }
    }

    candidates.sort_by_key(|(priority, index)| (*priority, *index));
    let mut selected = BTreeMap::<usize, ()>::new();
    for (_, index) in candidates.into_iter().take(budget) {
        selected.insert(index, ());
    }
    selected
}

fn git_diff_intent_context_radius() -> usize {
    2
}

fn is_git_diff_changed_detail_line(line: &str) -> bool {
    (line.starts_with('+') && !line.starts_with("+++")
        || line.starts_with('-') && !line.starts_with("---"))
        && !line
            .trim_start_matches(|ch| matches!(ch, '+' | '-'))
            .trim()
            .is_empty()
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
        "sum: git log --stat commits={}, stat_files={}",
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
    let (files, other) = collect_search_output_matches(input);

    let total_matches = files.values().map(Vec::len).sum::<usize>();
    if total_matches == 0 {
        return smart_truncate_command_output(input, options);
    }

    let mut output = Vec::new();
    output.push(format!(
        "sum: search matches={}, files={}",
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
    let entries = collect_file_list_entries(input);

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
    output.push(format!("sum: files entries={}", entries.len()));
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

fn collect_search_output_matches(input: &str) -> (BTreeMap<String, Vec<SearchMatch>>, Vec<String>) {
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

    (files, other)
}

fn collect_file_list_entries(input: &str) -> Vec<String> {
    command_lines(input)
        .into_iter()
        .filter_map(parse_file_list_entry_line)
        .collect()
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
        "sum: git diff stat_only entries={}",
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
    let preserved = collect_log_stream_preserved_lines(&lines);
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

const STRUCTURED_JSON_SUMMARY_MAX_VISITED_NODES: usize = 2048;
const STRUCTURED_JSON_SUMMARY_MAX_DEPTH: usize = 8;
const STRUCTURED_JSON_SUMMARY_MAX_SAMPLES: usize = 8;
const STRUCTURED_JSON_SUMMARY_MAX_KEYS: usize = 16;
const STRUCTURED_JSON_NDJSON_MIN_RECORDS: usize = 2;

#[derive(Debug, Default)]
struct StructuredJsonSummary {
    visited_nodes: usize,
    object_count: usize,
    array_count: usize,
    scalar_count: usize,
    key_counts: BTreeMap<String, usize>,
    level_counts: BTreeMap<String, usize>,
    error_samples: Vec<String>,
    path_samples: Vec<String>,
    id_samples: Vec<String>,
}

fn compact_structured_json_output(
    input: &str,
    options: &CommandOutputCompactOptions,
) -> Option<String> {
    compact_whole_json_output(input, options).or_else(|| compact_ndjson_output(input, options))
}

fn compact_whole_json_output(input: &str, options: &CommandOutputCompactOptions) -> Option<String> {
    let trimmed = input.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return None;
    }

    let value = serde_json::from_str::<Value>(trimmed).ok()?;
    let mut summary = StructuredJsonSummary::default();
    summarize_structured_json_value(&value, 0, &mut summary);

    let mut output = Vec::new();
    output.push(format!("pcs: json ({}->sum)", count_text_lines(input)));
    output.push(match &value {
        Value::Object(map) => format!("shape: object keys={}", map.len()),
        Value::Array(values) => format!("shape: array items={}", values.len()),
        Value::String(_) => "shape: string".to_string(),
        Value::Number(_) => "shape: number".to_string(),
        Value::Bool(_) => "shape: bool".to_string(),
        Value::Null => "shape: null".to_string(),
    });
    push_structured_json_summary_lines(&mut output, &summary, options);
    finalize_structured_json_compaction(input, output, options)
}

fn compact_ndjson_output(input: &str, options: &CommandOutputCompactOptions) -> Option<String> {
    let lines = command_lines(input);
    let non_empty = lines.iter().filter(|line| !line.trim().is_empty()).count();
    if non_empty < STRUCTURED_JSON_NDJSON_MIN_RECORDS {
        return None;
    }

    let mut parsed = 0usize;
    let mut summary = StructuredJsonSummary::default();
    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        parsed = parsed.saturating_add(1);
        summarize_structured_json_value(&value, 0, &mut summary);
    }

    if parsed < STRUCTURED_JSON_NDJSON_MIN_RECORDS || parsed.saturating_mul(2) < non_empty {
        return None;
    }

    let mut output = Vec::new();
    output.push(format!("pcs: ndjson ({}->sum)", count_text_lines(input)));
    output.push(format!(
        "sum: ndjson records={}, non_json={}",
        parsed,
        non_empty.saturating_sub(parsed)
    ));
    push_structured_json_summary_lines(&mut output, &summary, options);
    finalize_structured_json_compaction(input, output, options)
}

fn summarize_structured_json_value(
    value: &Value,
    depth: usize,
    summary: &mut StructuredJsonSummary,
) {
    if summary.visited_nodes >= STRUCTURED_JSON_SUMMARY_MAX_VISITED_NODES
        || depth > STRUCTURED_JSON_SUMMARY_MAX_DEPTH
    {
        return;
    }
    summary.visited_nodes = summary.visited_nodes.saturating_add(1);

    match value {
        Value::Object(map) => {
            summary.object_count = summary.object_count.saturating_add(1);
            for (key, child) in map {
                increment_count(&mut summary.key_counts, key);
                collect_structured_json_key_sample(key, child, summary);
                summarize_structured_json_value(child, depth.saturating_add(1), summary);
            }
        }
        Value::Array(values) => {
            summary.array_count = summary.array_count.saturating_add(1);
            for child in values {
                summarize_structured_json_value(child, depth.saturating_add(1), summary);
                if summary.visited_nodes >= STRUCTURED_JSON_SUMMARY_MAX_VISITED_NODES {
                    break;
                }
            }
        }
        _ => {
            summary.scalar_count = summary.scalar_count.saturating_add(1);
        }
    }
}

fn collect_structured_json_key_sample(
    key: &str,
    value: &Value,
    summary: &mut StructuredJsonSummary,
) {
    let lower = key.to_ascii_lowercase();
    if matches!(lower.as_str(), "level" | "severity" | "status" | "state")
        && let Some(label) = structured_json_scalar_string(value)
    {
        let label = label.to_ascii_lowercase();
        increment_count(&mut summary.level_counts, &label);
        if matches!(label.as_str(), "error" | "fatal" | "failed" | "failure") {
            let sample = format!("error sample: {key}={label}");
            push_structured_json_sample(&mut summary.error_samples, sample);
        }
    }

    if structured_json_key_is_error_like(&lower) {
        let sample = format!(
            "error sample: \"{}\"={}",
            key,
            structured_json_value_sample(value)
        );
        push_structured_json_sample(&mut summary.error_samples, sample);
    }

    if structured_json_key_is_path_like(&lower)
        && let Some(label) = structured_json_scalar_string(value)
    {
        let sample = format!("{key}={label}");
        push_structured_json_sample(&mut summary.path_samples, sample);
    }

    if structured_json_key_is_id_like(&lower)
        && let Some(label) = structured_json_scalar_string(value)
    {
        let sample = format!("{key}={label}");
        push_structured_json_sample(&mut summary.id_samples, sample);
    }
}

fn push_structured_json_summary_lines(
    output: &mut Vec<String>,
    summary: &StructuredJsonSummary,
    options: &CommandOutputCompactOptions,
) {
    output.push(format!(
        "nodes: objects={}, arrays={}, scalars={}, visited={}",
        summary.object_count, summary.array_count, summary.scalar_count, summary.visited_nodes,
    ));
    if !summary.key_counts.is_empty() {
        output.push(format_count_map(
            "keys",
            &summary.key_counts,
            STRUCTURED_JSON_SUMMARY_MAX_KEYS,
        ));
    }
    if !summary.level_counts.is_empty() {
        output.push(format_count_map("levels", &summary.level_counts, 10));
    }
    push_structured_json_sample_lines(
        output,
        "error samples",
        &summary.error_samples,
        options.max_line_chars,
    );
    push_structured_json_sample_lines(
        output,
        "path samples",
        &summary.path_samples,
        options.max_line_chars,
    );
    push_structured_json_sample_lines(
        output,
        "id samples",
        &summary.id_samples,
        options.max_line_chars,
    );
}

fn push_structured_json_sample_lines(
    output: &mut Vec<String>,
    label: &str,
    samples: &[String],
    max_line_chars: usize,
) {
    if samples.is_empty() {
        return;
    }
    output.push(format!("{label}:"));
    for sample in samples.iter().take(STRUCTURED_JSON_SUMMARY_MAX_SAMPLES) {
        output.push(format!(
            "  {}",
            truncate_command_line(sample, max_line_chars)
        ));
    }
    if samples.len() > STRUCTURED_JSON_SUMMARY_MAX_SAMPLES {
        output.push(format!(
            "  [... {} more samples ...]",
            samples.len() - STRUCTURED_JSON_SUMMARY_MAX_SAMPLES,
        ));
    }
}

fn finalize_structured_json_compaction(
    original: &str,
    lines: Vec<String>,
    options: &CommandOutputCompactOptions,
) -> Option<String> {
    let candidate = lines_to_text(lines);
    let candidate =
        ensure_no_critical_signal_loss_for_structured_json(original, &candidate, options);
    if candidate.len() < original.len() && critical_signal_self_check(original, &candidate).passed()
    {
        Some(candidate)
    } else {
        None
    }
}

fn ensure_no_critical_signal_loss_for_structured_json(
    original: &str,
    candidate: &str,
    options: &CommandOutputCompactOptions,
) -> String {
    if critical_signal_self_check(original, candidate).passed() {
        return candidate.to_string();
    }

    let mut output = command_lines(candidate)
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    output.push("critical signal rescue:".to_string());
    for line in command_lines(original) {
        if !is_critical_preserve_line(line) {
            continue;
        }
        output.push(truncate_command_line(line.trim(), options.max_line_chars));
        let text = lines_to_text(output.clone());
        if critical_signal_self_check(original, &text).passed() {
            return text;
        }
    }

    lines_to_text(output)
}

fn structured_json_scalar_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null => Some("null".to_string()),
        _ => None,
    }
}

fn structured_json_value_sample(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => "null".to_string(),
        Value::Object(map) => {
            let mut parts = Vec::new();
            for key in [
                "code", "type", "kind", "status", "message", "path", "file", "id",
            ] {
                if let Some(value) = map.get(key).and_then(structured_json_scalar_string) {
                    parts.push(format!("{key}={value}"));
                }
            }
            if parts.is_empty() {
                format!("object(keys={})", map.len())
            } else {
                parts.join(", ")
            }
        }
        Value::Array(values) => format!("array(items={})", values.len()),
    }
}

fn structured_json_key_is_error_like(lower: &str) -> bool {
    matches!(
        lower,
        "error" | "errors" | "err" | "exception" | "failure" | "failures" | "fatal"
    )
}

fn structured_json_key_is_path_like(lower: &str) -> bool {
    matches!(
        lower,
        "path" | "file" | "filename" | "filepath" | "source" | "target" | "uri" | "url" | "cwd"
    )
}

fn structured_json_key_is_id_like(lower: &str) -> bool {
    lower == "id" || lower.ends_with("_id") || lower.ends_with("id")
}

fn push_structured_json_sample(samples: &mut Vec<String>, sample: String) {
    if samples.len() >= STRUCTURED_JSON_SUMMARY_MAX_SAMPLES
        || samples.iter().any(|existing| existing == &sample)
    {
        return;
    }
    samples.push(sample);
}

fn increment_count(counts: &mut BTreeMap<String, usize>, key: &str) {
    counts
        .entry(key.to_string())
        .and_modify(|count| *count = count.saturating_add(1))
        .or_insert(1);
}

pub fn compact_successful_command_output_with_options(
    input: &str,
    options: &CommandSuccessOutputCompactOptions,
) -> CommandSuccessOutputCompactReport {
    let normalized = normalize_command_output(input);
    let lines = command_lines(&normalized);
    let original_lines = count_text_lines(&normalized);
    let critical_signals = count_critical_signals(&normalized);
    let failure_suspected =
        command_success_output_failure_suspected(&lines, critical_signals, options);

    if normalized.is_empty() || failure_suspected {
        return CommandSuccessOutputCompactReport {
            compacted: false,
            failure_suspected,
            original_lines,
            compacted_lines: original_lines,
            touched_files: collect_success_output_touched_files(&lines).len(),
            critical_signals,
            output: normalized,
        };
    }

    let success_like = command_success_output_success_like(&lines, options);
    if !success_like || original_lines < options.min_lines_to_compact.max(1) {
        return CommandSuccessOutputCompactReport {
            compacted: false,
            failure_suspected: false,
            original_lines,
            compacted_lines: original_lines,
            touched_files: collect_success_output_touched_files(&lines).len(),
            critical_signals,
            output: normalized,
        };
    }

    let touched_files = collect_success_output_touched_files(&lines);
    let mut noise_counts = BTreeMap::<String, usize>::new();
    let mut key_lines = Vec::<String>::new();
    for line in &lines {
        if let Some(label) = noisy_success_label(line) {
            *noise_counts.entry(label.to_string()).or_default() += 1;
        }
        if is_noisy_success_key_line(line) || is_rust_success_summary_line(line) {
            push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
        }
    }

    let non_empty = lines.iter().filter(|line| !line.trim().is_empty()).count();
    let noisy_success_lines = noise_counts.values().sum::<usize>();
    let mut output = Vec::<String>::new();
    output.push(format!("pcs: success cmd ({}->sum)", original_lines));
    output.push(format!(
        "command: {}",
        options.command.as_deref().unwrap_or("(unknown)")
    ));
    output.push(format!(
        "exit: {}",
        options
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "(unknown)".to_string())
    ));
    output.push(format!(
        "counts: lines={}, non_empty={}, noisy_success_lines={}, critical_signals={}, touched_files={}",
        original_lines,
        non_empty,
        noisy_success_lines,
        critical_signals.total(),
        touched_files.len()
    ));
    if !noise_counts.is_empty() {
        output.push(format_count_map("noise", &noise_counts, 10));
    }

    let include_path_summary =
        command_success_output_path_summary_useful(&lines, &touched_files, options);
    if include_path_summary {
        let roots = count_success_output_path_roots(&touched_files);
        if !roots.is_empty() {
            output.push(format_count_map("top roots", &roots, 8));
        }
        let extensions = count_success_output_path_extensions(&touched_files);
        if !extensions.is_empty() {
            output.push(format_count_map("extensions", &extensions, 8));
        }
    }

    if command_success_output_can_use_short_success_summary(
        critical_signals,
        include_path_summary,
        touched_files.len(),
        &key_lines,
        options,
    ) {
        let mut short = Vec::<String>::new();
        short.push(format!(
            "pcs: ok lines={} noisy={} touched={}",
            original_lines,
            noisy_success_lines,
            touched_files.len()
        ));
        if !noise_counts.is_empty() {
            short.push(format_count_map("noise", &noise_counts, 6));
        }
        if let Some(command) = options
            .command
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            short.push(format!(
                "cmd: {}",
                truncate_command_line(command, options.max_line_chars)
            ));
        }
        let output = lines_to_text(short);
        return CommandSuccessOutputCompactReport {
            compacted: true,
            failure_suspected: false,
            original_lines,
            compacted_lines: count_text_lines(&output),
            touched_files: touched_files.len(),
            critical_signals,
            output,
        };
    }

    push_labeled_lines(
        &mut output,
        "key lines",
        &key_lines,
        options.max_key_lines.max(1),
    );
    if include_path_summary {
        push_success_output_touched_files(&mut output, &touched_files, options);
    }

    let output = lines_to_text(output);
    let output =
        canonicalize_compacted_command_paths(&normalized, &output, CommandOutputKind::NoisySuccess);
    CommandSuccessOutputCompactReport {
        compacted: true,
        failure_suspected: false,
        original_lines,
        compacted_lines: count_text_lines(&output),
        touched_files: touched_files.len(),
        critical_signals,
        output,
    }
}

fn command_success_output_can_use_short_success_summary(
    critical_signals: CriticalSignalCounts,
    include_path_summary: bool,
    touched_files: usize,
    key_lines: &[String],
    options: &CommandSuccessOutputCompactOptions,
) -> bool {
    let short_candidate = options
        .command
        .as_deref()
        .is_some_and(command_name_is_short_success_output_candidate);

    if critical_signals.total() != 0
        || include_path_summary && !short_candidate
        || !key_lines
            .iter()
            .all(|line| noisy_success_label(line).is_some() || is_rust_success_summary_line(line))
    {
        return false;
    }

    touched_files == 0 || short_candidate
}

fn command_success_output_failure_suspected(
    lines: &[&str],
    critical_signals: CriticalSignalCounts,
    options: &CommandSuccessOutputCompactOptions,
) -> bool {
    options.exit_code.is_some_and(|code| code != 0)
        || !critical_signals.is_empty()
        || lines.iter().any(|line| {
            is_error_signal_line(line)
                || is_test_failure_signal_line(line)
                || is_success_output_failure_signal_line(line)
                || is_success_output_warning_signal_line(line)
                || is_diagnostic_failure_summary_line(line)
        })
}

fn command_success_output_success_like(
    lines: &[&str],
    options: &CommandSuccessOutputCompactOptions,
) -> bool {
    options.exit_code == Some(0)
        || looks_like_noisy_success_output(lines)
        || options
            .command
            .as_deref()
            .is_some_and(command_name_is_success_output_candidate)
}

fn command_name_is_success_output_candidate(command: &str) -> bool {
    let tokens = command_metadata_tokens(command);
    for index in 0..tokens.len() {
        let command = command_metadata_token_command_name(&tokens[index]);
        if matches!(
            command,
            "cargo"
                | "npm"
                | "pnpm"
                | "yarn"
                | "bun"
                | "corepack"
                | "make"
                | "cmake"
                | "ninja"
                | "ls"
                | "find"
                | "tree"
                | "du"
                | "tar"
                | "unzip"
                | "pip"
                | "pip3"
                | "uv"
                | "pipenv"
                | "poetry"
                | "ruff"
                | "mypy"
                | "biome"
                | "oxlint"
                | "pytest"
                | "py.test"
                | "swift"
                | "zig"
                | "tsc"
                | "vitest"
                | "jest"
                | "eslint"
                | "prettier"
                | "vite"
                | "next"
                | "playwright"
                | "cypress"
                | "nyc"
                | "c8"
                | "mvn"
                | "mvnw"
                | "gradle"
                | "gradlew"
                | "bazel"
                | "bazelisk"
                | "nx"
                | "turbo"
                | "docker-compose"
        ) {
            return true;
        }
        if command.ends_with("-tsc") || command.ends_with("_tsc") {
            return true;
        }
        if command == "go"
            && command_metadata_subcommand_after(&tokens, index)
                .is_some_and(|subcommand| matches!(subcommand, "test" | "build" | "vet" | "list"))
        {
            return true;
        }
        if command == "docker"
            && command_metadata_subcommand_after(&tokens, index).is_some_and(|subcommand| {
                matches!(subcommand, "build" | "buildx" | "pull" | "compose")
            })
        {
            return true;
        }
    }
    false
}

fn command_name_is_short_success_output_candidate(command: &str) -> bool {
    let tokens = command_metadata_tokens(command);
    for index in 0..tokens.len() {
        let command = command_metadata_token_command_name(&tokens[index]);
        if matches!(
            command,
            "tsc"
                | "vitest"
                | "jest"
                | "vite"
                | "next"
                | "playwright"
                | "cypress"
                | "biome"
                | "oxlint"
        ) || command.ends_with("-tsc")
            || command.ends_with("_tsc")
        {
            return true;
        }
        if command == "cargo"
            && command_metadata_subcommand_after(&tokens, index).is_some_and(|subcommand| {
                matches!(subcommand, "clippy" | "doc" | "fmt" | "fix" | "nextest")
            })
        {
            return true;
        }
        if matches!(command, "bun" | "swift" | "zig")
            || command == "uv"
                && command_metadata_subcommand_after(&tokens, index) == Some("run")
                && tokens
                    .iter()
                    .skip(index + 2)
                    .map(|token| command_metadata_token_command_name(token))
                    .any(|token| matches!(token, "pytest" | "py.test"))
        {
            return true;
        }
    }
    false
}

fn command_success_output_path_summary_useful(
    lines: &[&str],
    paths: &[String],
    options: &CommandSuccessOutputCompactOptions,
) -> bool {
    !paths.is_empty()
        && (options
            .command
            .as_deref()
            .is_some_and(command_name_is_path_relevant_success_output)
            || lines.iter().any(|line| {
                parse_file_list_entry_line(line).is_some()
                    || parse_search_match_line(line).is_some()
                    || parse_rg_json_match_line(line).is_some()
            }))
}

fn command_name_is_path_relevant_success_output(command: &str) -> bool {
    let tokens = command_metadata_tokens(command);
    for index in 0..tokens.len() {
        let command = command_metadata_token_command_name(&tokens[index]);
        if matches!(command, "ls" | "find" | "tree" | "du" | "rg" | "grep") {
            return true;
        }
        if command == "go"
            && command_metadata_subcommand_after(&tokens, index)
                .is_some_and(|subcommand| matches!(subcommand, "list"))
        {
            return true;
        }
    }
    false
}

fn collect_success_output_touched_files(lines: &[&str]) -> Vec<String> {
    let mut paths = BTreeMap::<String, ()>::new();
    for line in lines {
        if let Some(path) = parse_file_list_entry_line(line) {
            insert_success_output_path(&mut paths, &path);
        }
        if let Some(search_match) =
            parse_search_match_line(line).or_else(|| parse_rg_json_match_line(line))
        {
            insert_success_output_path(&mut paths, &search_match.path);
        }
        for token in line.split_whitespace() {
            if let Some(path) = success_output_path_from_token(token) {
                insert_success_output_path(&mut paths, &path);
            }
        }
    }
    paths.into_keys().collect()
}

fn insert_success_output_path(paths: &mut BTreeMap<String, ()>, path: &str) {
    let normalized = path.replace('\\', "/");
    let normalized = context_noise_strip_path_location_suffix_supplement(&normalized)
        .trim_start_matches("a/")
        .trim_start_matches("b/")
        .trim_start_matches("./")
        .trim_matches(|ch: char| {
            matches!(
                ch,
                '"' | '\'' | '`' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
            )
        })
        .trim_matches('/')
        .to_string();
    if normalized.len() < 3 || normalized.contains("://") || normalized.contains(' ') {
        return;
    }
    if normalized.starts_with('-') || normalized == "." || normalized == ".." {
        return;
    }
    paths.insert(normalized, ());
}

fn success_output_path_from_token(token: &str) -> Option<String> {
    let token = token.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\'' | '`' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
        )
    });
    context_noise_normalize_path_token_supplement(token).or_else(|| {
        let normalized = context_noise_strip_path_location_suffix_supplement(token);
        looks_like_bare_path_entry(normalized).then(|| normalized.to_string())
    })
}

fn count_success_output_path_roots(paths: &[String]) -> BTreeMap<String, usize> {
    let mut roots = BTreeMap::<String, usize>::new();
    for path in paths {
        *roots.entry(top_level_path_segment(path)).or_default() += 1;
    }
    roots
}

fn count_success_output_path_extensions(paths: &[String]) -> BTreeMap<String, usize> {
    let mut extensions = BTreeMap::<String, usize>::new();
    for path in paths {
        *extensions.entry(path_extension_label(path)).or_default() += 1;
    }
    extensions
}

fn push_success_output_touched_files(
    output: &mut Vec<String>,
    paths: &[String],
    options: &CommandSuccessOutputCompactOptions,
) {
    if paths.is_empty() {
        return;
    }
    let limit = options.max_touched_files.max(1);
    output.push(format!("touched files ({}):", paths.len()));
    for path in paths.iter().take(limit) {
        output.push(format!(
            "  {}",
            truncate_command_line(path, options.max_line_chars)
        ));
    }
    if paths.len() > limit {
        output.push(format!(
            "  [... {} more touched files ...]",
            paths.len() - limit
        ));
    }
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

fn collect_log_stream_preserved_lines(lines: &[&str]) -> Vec<String> {
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
        while next < lines.len() && is_log_stream_stack_context_line(lines[next]) {
            preserved.push(lines[next].to_string());
            next += 1;
        }
        index = next;
    }
    preserved
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

fn is_junit_xml_failure_line(line: &str) -> bool {
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

fn has_zero_only_summary_count(lower: &str, words: &[&str]) -> bool {
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

fn is_eslint_diagnostic_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    count_file_location_signals(trimmed) > 0
        && (lower.contains("  error  ")
            || lower.contains("  warning  ")
            || lower.contains(": error ")
            || lower.contains(": warning ")
            || lower.contains(" eslint "))
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

#[derive(Debug, Clone)]
struct IntentMatchLine {
    line_number: usize,
    text: String,
    score: usize,
}

fn normalize_intent_terms(terms: &[String]) -> Vec<String> {
    let mut seen = BTreeMap::<String, ()>::new();
    let mut normalized = Vec::new();
    for term in terms {
        let term = normalize_intent_match_text(term.trim());
        if term.is_empty() || seen.contains_key(&term) {
            continue;
        }
        seen.insert(term.clone(), ());
        normalized.push(term);
    }
    normalized
}

fn push_prompt_intent_candidate(
    terms: &mut Vec<String>,
    seen: &mut BTreeMap<String, ()>,
    candidate: &str,
    quoted: bool,
) {
    if terms.len() >= MAX_EXTRACTED_INTENT_TERMS {
        return;
    }
    if quoted && candidate.chars().any(char::is_whitespace) {
        for part in candidate.split_whitespace() {
            push_prompt_intent_candidate(terms, seen, part, false);
            if terms.len() >= MAX_EXTRACTED_INTENT_TERMS {
                break;
            }
        }
        return;
    }

    let Some(candidate) = normalize_prompt_intent_candidate(candidate) else {
        return;
    };

    let term = if let Some(code) = prompt_intent_error_code(&candidate) {
        Some(code)
    } else if let Some(path) = prompt_intent_path(&candidate) {
        Some(path)
    } else if looks_like_prompt_intent_file_name(&candidate)
        || looks_like_prompt_intent_symbol(&candidate)
        || quoted && looks_like_quoted_prompt_intent_identifier(&candidate)
    {
        Some(candidate)
    } else {
        None
    };

    let Some(term) = term else {
        return;
    };
    if term.chars().count() > 160 {
        return;
    }

    let key = normalize_intent_match_text(&term);
    if key.is_empty() || seen.contains_key(&key) {
        return;
    }
    seen.insert(key, ());
    terms.push(term);
}

fn normalize_prompt_intent_candidate(candidate: &str) -> Option<String> {
    let candidate = candidate.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\''
                | '`'
                | ','
                | ';'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '<'
                | '>'
                | '!'
                | '?'
                | '*'
                | '#'
                | '='
                | ':'
        )
    });
    let candidate = candidate.trim_end_matches(['.', ',']);
    if candidate.is_empty() || candidate.chars().any(char::is_whitespace) {
        return None;
    }

    Some(candidate.replace('\\', "/"))
}

fn is_prompt_intent_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | '\\' | ':' | '$' | '[' | ']')
}

fn prompt_intent_error_code(candidate: &str) -> Option<String> {
    let chars = candidate.chars().collect::<Vec<_>>();
    for index in 0..chars.len() {
        if chars[index].eq_ignore_ascii_case(&'e')
            && index + 4 < chars.len()
            && chars[index + 1..=index + 4]
                .iter()
                .all(|ch| ch.is_ascii_digit())
            && prompt_intent_code_boundary(&chars, index.checked_sub(1))
            && prompt_intent_code_boundary(&chars, Some(index + 5))
        {
            return Some(
                chars[index..=index + 4]
                    .iter()
                    .collect::<String>()
                    .to_ascii_uppercase(),
            );
        }

        if chars[index].eq_ignore_ascii_case(&'t')
            && index + 5 < chars.len()
            && chars[index + 1].eq_ignore_ascii_case(&'s')
            && chars[index + 2..=index + 5]
                .iter()
                .all(|ch| ch.is_ascii_digit())
            && prompt_intent_code_boundary(&chars, index.checked_sub(1))
            && prompt_intent_code_boundary(&chars, Some(index + 6))
        {
            return Some(
                chars[index..=index + 5]
                    .iter()
                    .collect::<String>()
                    .to_ascii_uppercase(),
            );
        }
    }
    None
}

fn prompt_intent_code_boundary(chars: &[char], index: Option<usize>) -> bool {
    index.is_none_or(|index| {
        chars
            .get(index)
            .is_none_or(|ch| !ch.is_ascii_alphanumeric() && *ch != '_')
    })
}

fn prompt_intent_path(candidate: &str) -> Option<String> {
    if candidate.contains("://") || candidate.contains(' ') {
        return None;
    }

    let stripped = context_noise_strip_path_location_suffix_supplement(candidate)
        .trim_start_matches("a/")
        .trim_start_matches("b/")
        .trim_start_matches("./")
        .trim_matches('/');
    if stripped.len() < 3 || !stripped.contains('/') || !looks_like_location_path(stripped) {
        return None;
    }

    Some(stripped.to_string())
}

fn looks_like_prompt_intent_file_name(candidate: &str) -> bool {
    if candidate.contains('/') || candidate.contains("://") || candidate.starts_with('-') {
        return false;
    }
    let Some((stem, ext)) = candidate.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && stem.chars().any(|ch| ch.is_ascii_alphanumeric())
        && !ext.is_empty()
        && ext.len() <= 12
        && ext
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn looks_like_prompt_intent_symbol(candidate: &str) -> bool {
    if candidate.contains("://") || candidate.len() < 3 {
        return false;
    }

    if candidate.contains("::") {
        return candidate
            .split("::")
            .all(is_prompt_intent_identifier_segment);
    }

    if candidate.contains('.') && !looks_like_prompt_intent_file_name(candidate) {
        let segments = candidate.split('.').collect::<Vec<_>>();
        return segments.len() >= 2
            && segments
                .iter()
                .all(|segment| is_prompt_intent_identifier_segment(segment));
    }

    if !is_prompt_intent_identifier_segment(candidate) {
        return false;
    }
    let lower = candidate.to_ascii_lowercase();
    if is_noisy_prompt_intent_word(&lower) {
        return false;
    }
    candidate.starts_with("test_")
        || candidate.ends_with("_test")
        || candidate.contains('_')
        || candidate
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch == '_' || ch.is_ascii_digit())
        || has_prompt_intent_camel_shape(candidate)
}

fn looks_like_quoted_prompt_intent_identifier(candidate: &str) -> bool {
    if !is_prompt_intent_identifier_segment(candidate) {
        return false;
    }
    !is_noisy_prompt_intent_word(&candidate.to_ascii_lowercase())
}

fn is_prompt_intent_identifier_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

fn has_prompt_intent_camel_shape(candidate: &str) -> bool {
    let uppercase = candidate
        .chars()
        .filter(|ch| ch.is_ascii_uppercase())
        .count();
    let lowercase = candidate.chars().any(|ch| ch.is_ascii_lowercase());
    if uppercase == 0 || !lowercase {
        return false;
    }

    let mut previous_lowercase = false;
    for ch in candidate.chars() {
        if previous_lowercase && ch.is_ascii_uppercase() {
            return true;
        }
        previous_lowercase = ch.is_ascii_lowercase();
    }
    uppercase >= 2
}

fn is_noisy_prompt_intent_word(word: &str) -> bool {
    matches!(
        word,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "for"
            | "from"
            | "in"
            | "into"
            | "is"
            | "it"
            | "of"
            | "on"
            | "or"
            | "the"
            | "this"
            | "to"
            | "with"
            | "without"
            | "should"
            | "when"
            | "please"
            | "fix"
            | "add"
            | "update"
            | "change"
            | "make"
            | "use"
            | "using"
            | "run"
            | "check"
            | "test"
            | "tests"
            | "error"
            | "failed"
            | "failure"
            | "warning"
            | "output"
            | "command"
            | "context"
            | "smart"
            | "tool"
            | "compaction"
            | "prompt"
            | "request"
            | "ignore"
            | "common"
            | "word"
            | "words"
            | "file"
            | "files"
            | "repo"
            | "path"
            | "paths"
            | "rust"
            | "python"
            | "javascript"
            | "typescript"
    )
}

fn compact_command_output_for_intent(
    original: &str,
    base_output: &str,
    kind: CommandOutputKind,
    options: &CommandOutputCompactOptions,
    intent_terms: &[String],
) -> String {
    if !command_output_kind_supports_intent_compaction(kind) {
        return base_output.to_string();
    }
    if kind == CommandOutputKind::Search {
        return compact_search_output_for_intent(original, base_output, options, intent_terms);
    }
    if kind == CommandOutputKind::FileList {
        return compact_file_list_output_for_intent(original, base_output, options, intent_terms);
    }
    if kind == CommandOutputKind::GitDiff {
        return compact_git_diff_output_with_intent(original, options, intent_terms);
    }

    let intent_matches = collect_intent_matching_lines(original, intent_terms, options);
    if intent_matches.is_empty() {
        return base_output.to_string();
    }

    let base_lines = command_lines(base_output);
    let max_lines = options.max_lines.saturating_add(1).max(6);
    let mut output = Vec::new();
    if let Some(header) = base_lines.first() {
        output.push((*header).to_string());
    } else {
        output.push(format!(
            "pcs: {} ({}->intent)",
            kind.label(),
            count_text_lines(original),
        ));
    }

    output.push(format!(
        "int: {} lines for {}",
        intent_matches.len(),
        truncate_command_line(&intent_terms.join(", "), options.max_line_chars),
    ));

    let reserved_for_baseline = 3usize.min(max_lines.saturating_sub(output.len()));
    let mut intent_budget = max_lines
        .saturating_sub(output.len())
        .saturating_sub(reserved_for_baseline)
        .max(1);
    intent_budget = intent_budget.min(max_lines.saturating_div(2).max(2));
    for intent_match in intent_matches.iter().take(intent_budget) {
        output.push(format!(
            "  L{}: {}",
            intent_match.line_number,
            truncate_command_line(&intent_match.text, options.max_line_chars),
        ));
    }
    if intent_matches.len() > intent_budget {
        output.push(format!(
            "  [... {} more intent-matching lines ...]",
            intent_matches.len() - intent_budget
        ));
    }

    let remaining = max_lines.saturating_sub(output.len());
    if remaining >= 2 && base_lines.len() > 1 {
        output.push("base:".to_string());
        let baseline_budget = max_lines.saturating_sub(output.len()).max(1);
        let baseline = base_lines
            .iter()
            .skip(1)
            .map(|line| (*line).to_string())
            .collect::<Vec<_>>();
        push_head_tail_lines(
            &mut output,
            &baseline,
            baseline_budget,
            options.max_line_chars,
            "baseline lines",
            "  ",
        );
    }

    lines_to_text(output)
}

fn compact_search_output_for_intent(
    original: &str,
    base_output: &str,
    options: &CommandOutputCompactOptions,
    intent_terms: &[String],
) -> String {
    let (files, _) = collect_search_output_matches(original);
    let total_matches = files.values().map(Vec::len).sum::<usize>();
    if total_matches == 0 {
        return base_output.to_string();
    }

    let mut relevant = Vec::<(usize, String, Vec<SearchMatch>)>::new();
    let mut other_files = 0usize;
    let mut other_matches = 0usize;
    for (path, matches) in files {
        let path_score = score_intent_text(&path, intent_terms).saturating_mul(4);
        let mut scored = matches
            .iter()
            .cloned()
            .map(|search_match| {
                let score =
                    path_score.saturating_add(score_intent_text(&search_match.text, intent_terms));
                (score, search_match)
            })
            .collect::<Vec<_>>();
        let file_score = scored.iter().map(|(score, _)| *score).max().unwrap_or(0);
        if file_score == 0 {
            other_files += 1;
            other_matches += matches.len();
            continue;
        }
        scored.sort_by_key(|(score, search_match)| {
            (
                Reverse(*score),
                search_match.line_number.unwrap_or(usize::MAX),
                search_match.text.clone(),
            )
        });
        relevant.push((
            file_score,
            path,
            scored
                .into_iter()
                .map(|(_, search_match)| search_match)
                .collect(),
        ));
    }

    if relevant.is_empty() {
        return base_output.to_string();
    }

    relevant.sort_by_key(|(score, path, _)| (Reverse(*score), path.clone()));
    let base_lines = command_lines(base_output);
    let max_lines = options.max_lines.saturating_add(1).max(8);
    let mut output = Vec::<String>::new();
    push_intent_header(
        &mut output,
        &base_lines,
        CommandOutputKind::Search,
        original,
    );
    let relevant_matches = relevant
        .iter()
        .map(|(_, _, matches)| matches.len())
        .sum::<usize>();
    output.push(format!(
        "int: {} search matches across {} files for {}",
        relevant_matches,
        relevant.len(),
        truncate_command_line(&intent_terms.join(", "), options.max_line_chars),
    ));
    output.push(format!(
        "overflow: {} other matches across {} files",
        other_matches, other_files,
    ));
    output.push("rel search:".to_string());

    let mut budget = max_lines.saturating_sub(output.len()).max(1);
    let reserve = usize::from(other_matches > 0).saturating_add(usize::from(base_lines.len() > 1));
    budget = budget.saturating_sub(reserve).max(1);
    let per_file = options.max_search_matches_per_file.max(1);
    let mut hidden_relevant = 0usize;
    for (_, path, matches) in &relevant {
        if budget <= 1 {
            hidden_relevant += matches.len();
            continue;
        }
        output.push(format!("{path} ({} relevant matches):", matches.len()));
        budget = budget.saturating_sub(1);
        let shown = matches.len().min(per_file).min(budget);
        for search_match in matches.iter().take(shown) {
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
        budget = budget.saturating_sub(shown);
        if matches.len() > shown {
            output.push(format!(
                "  [... {} more relevant matches in this file ...]",
                matches.len() - shown
            ));
            budget = budget.saturating_sub(1);
        }
    }
    if hidden_relevant > 0 {
        output.push(format!(
            "[... omitted {hidden_relevant} additional relevant search matches ...]"
        ));
    }
    push_intent_baseline_tail(&mut output, &base_lines, max_lines, options);
    lines_to_text(output)
}

fn compact_file_list_output_for_intent(
    original: &str,
    base_output: &str,
    options: &CommandOutputCompactOptions,
    intent_terms: &[String],
) -> String {
    let entries = collect_file_list_entries(original);
    if entries.is_empty() {
        return base_output.to_string();
    }

    let mut relevant = Vec::<(usize, String)>::new();
    let mut overflow = Vec::<String>::new();
    for entry in entries {
        let score = score_intent_text(&entry, intent_terms);
        if score == 0 {
            overflow.push(entry);
        } else {
            relevant.push((score, entry));
        }
    }
    if relevant.is_empty() {
        return base_output.to_string();
    }

    relevant.sort_by_key(|(score, path)| (Reverse(*score), path.clone()));
    let base_lines = command_lines(base_output);
    let max_lines = options.max_lines.saturating_add(1).max(8);
    let mut output = Vec::<String>::new();
    push_intent_header(
        &mut output,
        &base_lines,
        CommandOutputKind::FileList,
        original,
    );
    output.push(format!(
        "int: {} paths for {}",
        relevant.len(),
        truncate_command_line(&intent_terms.join(", "), options.max_line_chars),
    ));
    if !overflow.is_empty() {
        let roots = count_success_output_path_roots(&overflow);
        let extensions = count_success_output_path_extensions(&overflow);
        output.push(format!("overflow: {} other file entries", overflow.len()));
        output.push(format_count_map("overflow roots", &roots, 6));
        output.push(format_count_map("overflow extensions", &extensions, 6));
    }
    output.push("rel paths:".to_string());

    let mut budget = max_lines.saturating_sub(output.len()).max(1);
    let reserve = usize::from(base_lines.len() > 1);
    budget = budget.saturating_sub(reserve).max(1);
    let path_limit = options.max_path_entries.max(1).min(budget);
    for (_, path) in relevant.iter().take(path_limit) {
        output.push(format!(
            "  {}",
            truncate_command_line(path, options.max_line_chars)
        ));
    }
    if relevant.len() > path_limit {
        output.push(format!(
            "  [... {} more relevant paths ...]",
            relevant.len() - path_limit
        ));
    }
    push_intent_baseline_tail(&mut output, &base_lines, max_lines, options);
    lines_to_text(output)
}

fn push_intent_header(
    output: &mut Vec<String>,
    base_lines: &[&str],
    kind: CommandOutputKind,
    original: &str,
) {
    if let Some(header) = base_lines.first() {
        output.push((*header).to_string());
    } else {
        output.push(format!(
            "pcs: {} ({}->intent)",
            kind.label(),
            count_text_lines(original),
        ));
    }
}

fn push_intent_baseline_tail(
    output: &mut Vec<String>,
    base_lines: &[&str],
    max_lines: usize,
    options: &CommandOutputCompactOptions,
) {
    let remaining = max_lines.saturating_sub(output.len());
    if remaining < 3 || base_lines.len() <= 1 {
        return;
    }
    output.push("base:".to_string());
    let baseline = base_lines
        .iter()
        .skip(1)
        .map(|line| (*line).to_string())
        .collect::<Vec<_>>();
    push_head_tail_lines(
        output,
        &baseline,
        max_lines.saturating_sub(output.len()).max(1),
        options.max_line_chars,
        "baseline lines",
        "  ",
    );
}

fn score_intent_text(value: &str, intent_terms: &[String]) -> usize {
    let value = normalize_intent_match_text(value);
    intent_terms.iter().fold(0usize, |score, term| {
        if term.is_empty() {
            return score;
        }
        let mut term_score = 0usize;
        if value.contains(term.as_str()) {
            term_score = term_score
                .saturating_add(4)
                .saturating_add(term.len().min(48).saturating_div(8));
        }
        if let Some(basename) = intent_term_basename(term)
            && basename != term.as_str()
            && value.contains(basename)
        {
            term_score = term_score.saturating_add(2);
        }
        score.saturating_add(term_score)
    })
}

fn intent_term_basename(term: &str) -> Option<&str> {
    term.rsplit('/').next().filter(|basename| {
        !basename.is_empty() && basename.len() >= 3 && basename.len() < term.len()
    })
}

fn command_output_kind_supports_intent_compaction(kind: CommandOutputKind) -> bool {
    matches!(
        kind,
        CommandOutputKind::GitDiff
            | CommandOutputKind::RustDiagnostics
            | CommandOutputKind::Diagnostics
            | CommandOutputKind::Search
            | CommandOutputKind::FileList
            | CommandOutputKind::Plain
    )
}

fn collect_intent_matching_lines(
    input: &str,
    intent_terms: &[String],
    options: &CommandOutputCompactOptions,
) -> Vec<IntentMatchLine> {
    let lines = command_lines(input);
    let mut selected = BTreeMap::<usize, usize>::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let score = score_intent_text(trimmed, intent_terms);
        if score == 0 {
            continue;
        }
        selected
            .entry(index)
            .and_modify(|existing| *existing = (*existing).max(score))
            .or_insert(score);
        for nearby in intent_context_line_range(index, lines.len()) {
            if nearby == index || lines[nearby].trim().is_empty() {
                continue;
            }
            selected
                .entry(nearby)
                .and_modify(|existing| *existing = (*existing).max(score.saturating_sub(1)))
                .or_insert(score.saturating_sub(1).max(1));
        }
    }

    let mut matches = selected
        .into_iter()
        .map(|(index, score)| IntentMatchLine {
            line_number: index + 1,
            text: truncate_command_line(lines[index].trim(), options.max_line_chars),
            score,
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|line| (Reverse(line.score), line.line_number));
    matches
}

fn intent_line_matches(line: &str, intent_terms: &[String]) -> bool {
    score_intent_text(line, intent_terms) > 0
}

fn normalize_intent_match_text(value: &str) -> String {
    value.replace('\\', "/").trim().to_ascii_lowercase()
}

fn intent_context_line_range(index: usize, total: usize) -> std::ops::RangeInclusive<usize> {
    let start = index.saturating_sub(1);
    let end = (index + 1).min(total.saturating_sub(1));
    start..=end
}

fn ensure_no_critical_signal_loss_for_intent(
    original: &str,
    candidate: &str,
    options: &CommandOutputCompactOptions,
) -> String {
    if critical_signal_self_check(original, candidate).passed() {
        return candidate.to_string();
    }

    let mut output = command_lines(candidate)
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    output.push("critical signal rescue:".to_string());
    for line in command_lines(original) {
        if !is_critical_preserve_line(line) {
            continue;
        }
        output.push(truncate_command_line(line.trim(), options.max_line_chars));
        let text = lines_to_text(output.clone());
        if critical_signal_self_check(original, &text).passed() {
            return text;
        }
    }

    let text = lines_to_text(output);
    if critical_signal_self_check(original, &text).passed() {
        text
    } else {
        candidate.to_string()
    }
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

fn is_generated_compaction_header_line(line: &str) -> bool {
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

fn is_error_signal_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    if has_zero_only_summary_count(&lower, &["error", "errors"]) {
        return false;
    }
    if lower.starts_with("error:")
        || lower.starts_with("error[")
        || lower.starts_with("error ")
        || lower.starts_with("error\t")
        || lower.starts_with("fatal:")
        || lower.starts_with("panic:")
        || lower.starts_with("npm err!")
        || lower.starts_with("npm error")
        || lower.starts_with("pnpm error")
        || lower.starts_with("yarn error")
        || lower.starts_with("bun error")
        || lower.starts_with('#') && lower.contains(" error")
        || lower.starts_with("failed ")
        || lower.starts_with("fail ")
        || trimmed.starts_with("E   ")
        || lower.starts_with("thread '") && lower.contains("' panicked at")
        || is_rust_panic_line(line)
        || is_typescript_diagnostic_line(line)
        || is_eslint_diagnostic_line(line)
        || is_junit_xml_failure_line(line)
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
        .any(token_contains_paren_file_location)
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

    let trimmed = line
        .trim_start_matches(|ch| matches!(ch, '+' | '-' | ' '))
        .trim();
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

impl CommandOutputKind {
    fn label(self) -> &'static str {
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

fn is_static_duplicate_context_file(path: &Path) -> bool {
    is_compressible_context_file(path)
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
mod success_short_form_tests {
    use super::*;

    fn success_options(command: &str) -> CommandSuccessOutputCompactOptions {
        CommandSuccessOutputCompactOptions {
            command: Some(command.to_string()),
            exit_code: Some(0),
            min_lines_to_compact: 1,
            max_touched_files: 4,
            max_key_lines: 4,
            max_line_chars: 160,
        }
    }

    fn assert_short_success(report: &CommandSuccessOutputCompactReport, command: &str) {
        assert!(report.compacted, "{command}");
        assert!(!report.failure_suspected, "{command}");
        assert_eq!(report.critical_signals.total(), 0, "{command}");
        assert!(
            report.output.starts_with("pcs: ok "),
            "{command}: {}",
            report.output
        );
        assert!(report.output.contains("cmd:"), "{command}");
    }

    #[test]
    fn successful_command_output_short_forms_cargo_clippy_and_doc_noise() {
        let mut clippy = String::new();
        for index in 0..12 {
            clippy.push_str(&format!("    Checking crate_{index} v0.1.0\n"));
        }
        clippy.push_str("    Finished `dev` profile [unoptimized] target(s) in 1.23s\n");

        let report = compact_successful_command_output_with_options(
            &clippy,
            &success_options("cargo clippy --workspace --all-targets"),
        );
        assert_short_success(&report, "cargo clippy");
        assert!(report.output.contains("checking=12"));
        assert!(!report.output.contains("Checking crate_0"));

        let mut docs = String::new();
        for index in 0..12 {
            docs.push_str(&format!(" Documenting crate_{index} v0.1.0\n"));
        }
        docs.push_str("    Finished `dev` profile [unoptimized] target(s) in 2.34s\n");
        docs.push_str("   Generated /repo/target/doc/prodex/index.html\n");

        let report =
            compact_successful_command_output_with_options(&docs, &success_options("cargo doc"));
        assert_short_success(&report, "cargo doc");
        assert!(report.touched_files > 0);
        assert!(report.output.contains("documenting=12"));
        assert!(!report.output.contains("/repo/target/doc/prodex/index.html"));
    }

    #[test]
    fn successful_command_output_short_forms_common_frontend_success_noise() {
        let tsc = "\
Projects in this build:
    * tsconfig.json
Project 'tsconfig.json' is up to date because newest input 'src/app.ts' is older than output 'tsconfig.tsbuildinfo'
Found 0 errors.
";
        let vite = "\
vite v5.4.19 building for production...
transforming...
✓ 42 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                  0.45 kB │ gzip: 0.29 kB
dist/assets/index-abc.js        24.12 kB │ gzip: 8.00 kB
✓ built in 1.23s
";
        let next = "\
▲ Next.js 15.3.1
Creating an optimized production build ...
✓ Compiled successfully in 1000ms
Linting and checking validity of types ...
Collecting page data ...
Generating static pages (0/5) ...
✓ Generating static pages (5/5)
Finalizing page optimization ...
Collecting build traces ...
Route (app)                              Size     First Load JS
┌ ○ /                                 5.56 kB         105 kB
+ First Load JS shared by all          99.6 kB
";
        let playwright = "\
Running 3 tests using 2 workers
✓ home page renders (120ms)
✓ settings page renders (130ms)
✓ login page renders (140ms)
3 passed (1.2s)
";
        let cypress = "\
Spec                                              Tests  Passing  Failing  Pending  Skipped
tests/app.cy.ts                                      4        4        0        0        0
Passing: 4
Failing: 0
All specs passed!
";
        let jest = "\
PASS tests/unit_0.test.ts
PASS tests/unit_1.test.ts
PASS tests/unit_2.test.ts
Test Suites: 3 passed, 3 total
Tests:       18 passed, 18 total
Snapshots:   0 total
Time:        1.23 s
Ran all test suites.
";

        for (command, input, omitted_line) in [
            ("npx tsc --noEmit", tsc, "Project 'tsconfig.json'"),
            ("vite build", vite, "dist/assets/index-abc.js"),
            ("next build", next, "Route (app)"),
            ("playwright test", playwright, "home page renders"),
            ("cypress run", cypress, "tests/app.cy.ts"),
            ("jest --runInBand", jest, "PASS tests/unit_0.test.ts"),
        ] {
            let report =
                compact_successful_command_output_with_options(input, &success_options(command));
            assert_short_success(&report, command);
            assert!(!report.output.contains(omitted_line), "{command}");
        }
    }

    #[test]
    fn successful_command_output_short_form_refuses_common_tool_warnings_and_failures() {
        for (command, input) in [
            (
                "next build",
                "Compiled with warnings\napp/page.tsx imports a large dependency\n",
            ),
            (
                "cypress run",
                "Spec Tests Passing Failing Pending Skipped\nFailing: 1\n",
            ),
            (
                "next build",
                "Type error: Page \"app/page.tsx\" has an invalid default export\n",
            ),
        ] {
            let report =
                compact_successful_command_output_with_options(input, &success_options(command));
            assert!(!report.compacted, "{command}");
            assert!(report.failure_suspected, "{command}");
            assert_eq!(report.output, input, "{command}");
        }
    }
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
