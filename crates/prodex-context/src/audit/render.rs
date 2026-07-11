use terminal_ui::{fit_cell, section_header_with_width};

use super::duplicates::context_static_duplicate_location_label;
use super::types::{ContextAuditReport, ContextStaticDuplicateReport};

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

pub(crate) fn format_count<T: std::fmt::Display>(value: T) -> String {
    value.to_string()
}
