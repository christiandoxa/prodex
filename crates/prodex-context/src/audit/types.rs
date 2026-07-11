use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone)]
pub(super) struct ContextStaticDuplicateCandidate {
    pub(super) key: String,
    pub(super) preview: String,
    pub(super) occurrence: ContextStaticDuplicateOccurrence,
    pub(super) words: usize,
    pub(super) normalized_chars: usize,
    pub(super) estimated_tokens: usize,
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
