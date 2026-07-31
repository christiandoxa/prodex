mod duplicates;
mod files;
mod render;
mod types;

use std::cmp::Reverse;
use std::path::Path;

use anyhow::Result;
use redaction::redaction_display_os;

use crate::{estimate_context_tokens, is_compressible_context_file};

use duplicates::{
    build_context_static_duplicate_report, context_static_duplicate_candidates_for_text,
};
pub(crate) use files::collect_context_files;
use files::{
    CONTEXT_AUDIT_ROOTS, CONTEXT_WALK_MAX_BYTES, ContextReadRoot, collect_context_files_for_audit,
    is_auditable_context_file, is_static_duplicate_context_file, read_context_file_bounded,
};
pub(crate) use render::format_count;
pub use render::render_context_audit_report_with_width;
pub use types::{
    ContextAuditEntry, ContextAuditError, ContextAuditReport, ContextCompressEntry,
    ContextCompressReport, ContextStaticDuplicateOccurrence, ContextStaticDuplicateReport,
    ContextStaticDuplicateSnippet,
};

const CONTEXT_AUDIT_MAX_ERRORS: usize = 64;

pub fn collect_context_static_duplicate_report(
    root: &Path,
    limit: usize,
) -> Result<ContextStaticDuplicateReport> {
    duplicates::collect_context_static_duplicate_report(root, limit)
}

fn context_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn collect_context_audit_report(root: &Path, limit: usize) -> Result<ContextAuditReport> {
    let mut paths = Vec::new();
    let mut errors = Vec::new();
    let mut hidden_errors = 0_usize;
    let read_root = ContextReadRoot::open(root)?;
    if read_root.is_some() {
        let mut record_traversal_error = |path: &Path| {
            record_context_audit_error(
                root,
                path,
                "traversal",
                "could not inspect part of the context tree; check permissions or whether it changed during the audit",
                &mut errors,
                &mut hidden_errors,
            );
        };
        for entry in CONTEXT_AUDIT_ROOTS {
            collect_context_files_for_audit(
                &root.join(entry),
                &mut paths,
                &mut record_traversal_error,
            )?;
        }
    }
    paths.sort();
    paths.dedup();

    let mut files = Vec::new();
    let mut duplicate_candidates = Vec::new();
    let mut read_bytes = 0_u64;
    for path in paths {
        let is_auditable = match is_auditable_context_file(&path) {
            Ok(is_auditable) => is_auditable,
            Err(_) => {
                record_context_audit_error(
                    root,
                    &path,
                    "metadata",
                    "could not inspect file metadata; check permissions or whether the file was removed or changed during the audit",
                    &mut errors,
                    &mut hidden_errors,
                );
                continue;
            }
        };
        if !is_auditable {
            continue;
        }
        let Some(read_root) = read_root.as_ref() else {
            continue;
        };
        read_root.validate()?;
        let opened = read_context_file_bounded(read_root, &path);
        read_root.validate()?;
        let opened = match opened {
            Ok(opened) => opened,
            Err(_) => {
                record_context_audit_error(
                    root,
                    &path,
                    "read",
                    "could not read bounded UTF-8 content; check permissions, file size, encoding, or whether the file changed during the audit",
                    &mut errors,
                    &mut hidden_errors,
                );
                continue;
            }
        };
        read_bytes = read_bytes.saturating_add(opened.text.len() as u64);
        if read_bytes > CONTEXT_WALK_MAX_BYTES {
            anyhow::bail!("context audit read limit exceeded at {}", safe_path(&path));
        }
        let text = opened.text;
        let chars = text.chars().count();
        let words = text.split_whitespace().count();
        let estimated_tokens = estimate_context_tokens(chars, words);
        let relative_path = context_relative_path(root, &path);
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
            bytes: opened.metadata.len(),
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
        errors,
        hidden_errors,
        static_duplicates,
    })
}

pub(super) fn safe_path(path: &Path) -> String {
    redaction_display_os(path.as_os_str())
}

fn record_context_audit_error(
    root: &Path,
    path: &Path,
    operation: &str,
    message: &str,
    errors: &mut Vec<ContextAuditError>,
    hidden_errors: &mut usize,
) {
    if errors.len() >= CONTEXT_AUDIT_MAX_ERRORS {
        *hidden_errors = hidden_errors.saturating_add(1);
        return;
    }
    errors.push(ContextAuditError {
        relative_path: context_audit_error_path(root, path),
        operation: operation.to_string(),
        message: message.to_string(),
    });
}

fn context_audit_error_path(root: &Path, path: &Path) -> String {
    let relative = path
        .strip_prefix(root)
        .ok()
        .filter(|path| !path.as_os_str().is_empty())
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| "<outside audit root>".to_string());
    relative.chars().flat_map(char::escape_default).collect()
}
