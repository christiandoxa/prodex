use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::estimate_context_tokens;

use super::files::{CONTEXT_AUDIT_ROOTS, collect_context_files, is_static_duplicate_context_file};
use super::types::{
    ContextStaticDuplicateCandidate, ContextStaticDuplicateOccurrence,
    ContextStaticDuplicateReport, ContextStaticDuplicateSnippet,
};

const STATIC_DUPLICATE_MIN_CHARS: usize = 80;
const STATIC_DUPLICATE_MIN_WORDS: usize = 10;
const STATIC_DUPLICATE_PREVIEW_CHARS: usize = 160;

pub(super) fn collect_context_static_duplicate_report(
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

pub(super) fn context_static_duplicate_candidates_for_text(
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

pub(super) fn build_context_static_duplicate_report(
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

pub(super) fn context_static_duplicate_location_label(
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
