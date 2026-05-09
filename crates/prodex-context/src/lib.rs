use anyhow::{Context, Result};
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use terminal_ui::{fit_cell, section_header, section_header_with_width};

mod blob_noise;
mod command_output;
#[path = "lib/compression.rs"]
mod compression;
mod critical_signal;
pub use blob_noise::{
    ContextBlobNoiseFinding, ContextBlobNoiseKind, ContextBlobNoiseReport,
    detect_context_blob_noise, detect_context_blob_noise_for_path, is_context_blob_noise,
};
#[cfg(test)]
pub(crate) use command_output::is_generated_compaction_header_line;
pub use command_output::{
    CommandOutputCompactOptions, CommandOutputCompactReport, CommandOutputIntentCompactOptions,
    CommandOutputKind, CommandSuccessOutputCompactOptions, CommandSuccessOutputCompactReport,
    MAX_EXTRACTED_INTENT_TERMS, command_output_kind_hint_for_command, compact_command_output,
    compact_command_output_with_intent_options, compact_command_output_with_intent_terms,
    compact_command_output_with_options, compact_command_output_with_options_and_kind_hint,
    compact_successful_command_output_with_options, extract_intent_terms_from_prompt,
    infer_command_output_kind_from_metadata,
};
pub(crate) use command_output::{
    command_lines, count_text_lines, generic_failed_test_name, has_zero_only_summary_count,
    is_eslint_diagnostic_line, is_exception_signal_line, is_junit_xml_failure_line,
    is_log_level_signal_line, is_rust_backtrace_start, is_rust_exit_status_line,
    is_rust_failure_summary_line, is_rust_panic_line, is_typescript_diagnostic_line,
    normalize_command_output, parse_file_list_entry_line, parse_rg_json_match_line,
    parse_search_match_line, rust_diagnostic_severity, rust_failed_test_name,
    rust_failure_separator_name,
};
pub use compression::{
    compress_context_path, compress_context_text, render_context_compress_report,
};
pub(crate) use compression::{
    estimate_context_tokens, is_compressible_context_file, is_context_backup,
};
pub use critical_signal::{
    CriticalSignalCounts, CriticalSignalLineRange, CriticalSignalLineRangeOptions,
    CriticalSignalSelfCheck, count_critical_signals, critical_signal_lost_line_ranges,
    critical_signal_lost_line_ranges_with_options, critical_signal_self_check,
};

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
