mod duplicates;
mod files;
mod render;
mod types;

use std::cmp::Reverse;
use std::path::Path;

use anyhow::Result;

use crate::{estimate_context_tokens, is_compressible_context_file};

use duplicates::{
    build_context_static_duplicate_report, context_static_duplicate_candidates_for_text,
};
pub(crate) use files::collect_context_files;
use files::{
    CONTEXT_AUDIT_ROOTS, CONTEXT_WALK_MAX_BYTES, ContextReadRoot, is_auditable_context_file,
    is_static_duplicate_context_file, read_context_file_bounded,
};
pub(crate) use render::format_count;
pub use render::render_context_audit_report_with_width;
pub use types::{
    ContextAuditEntry, ContextAuditReport, ContextCompressEntry, ContextCompressReport,
    ContextStaticDuplicateOccurrence, ContextStaticDuplicateReport, ContextStaticDuplicateSnippet,
};

pub fn collect_context_static_duplicate_report(
    root: &Path,
    limit: usize,
) -> Result<ContextStaticDuplicateReport> {
    duplicates::collect_context_static_duplicate_report(root, limit)
}

pub fn collect_context_audit_report(root: &Path, limit: usize) -> Result<ContextAuditReport> {
    let mut paths = Vec::new();
    let read_root = ContextReadRoot::open(root)?;
    if read_root.is_some() {
        for entry in CONTEXT_AUDIT_ROOTS {
            collect_context_files(&root.join(entry), &mut paths)?;
        }
    }
    paths.sort();
    paths.dedup();

    let mut files = Vec::new();
    let mut duplicate_candidates = Vec::new();
    let mut read_bytes = 0_u64;
    for path in paths {
        if !is_auditable_context_file(&path) {
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
            Err(_) => continue,
        };
        read_bytes = read_bytes.saturating_add(opened.text.len() as u64);
        if read_bytes > CONTEXT_WALK_MAX_BYTES {
            anyhow::bail!("context audit read limit exceeded at {}", path.display());
        }
        let text = opened.text;
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
        static_duplicates,
    })
}
