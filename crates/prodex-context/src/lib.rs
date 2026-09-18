use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use terminal_ui::section_header;

mod audit;
mod blob_noise;
mod command_output;
#[path = "lib/compression.rs"]
mod compression;
mod critical_signal;
pub use audit::{
    ContextAuditEntry, ContextAuditError, ContextAuditReport, ContextCompressEntry,
    ContextCompressReport, ContextStaticDuplicateOccurrence, ContextStaticDuplicateReport,
    ContextStaticDuplicateSnippet, collect_context_audit_report,
    collect_context_static_duplicate_report, render_context_audit_report_with_width,
};
pub(crate) use audit::{collect_context_files, format_count};
pub use blob_noise::{
    ContextBlobNoiseFinding, ContextBlobNoiseKind, ContextBlobNoiseReport,
    detect_context_blob_noise, detect_context_blob_noise_for_path, is_context_blob_noise,
};
#[cfg(test)]
pub(crate) use command_output::is_generated_compaction_header_line;
pub use command_output::{
    CommandOutputCompactLimits, CommandOutputCompactOptions, CommandOutputCompactReport,
    CommandOutputIntentCompactOptions, CommandOutputKind, MAX_EXTRACTED_INTENT_TERMS,
    command_output_kind_hint_for_command, compact_command_output,
    compact_command_output_with_intent_options, compact_command_output_with_intent_terms,
    compact_command_output_with_options, compact_command_output_with_options_and_kind_hint,
    extract_intent_terms_from_prompt, infer_command_output_kind_from_metadata,
};
pub(crate) use command_output::{
    command_lines, count_text_lines, normalize_command_output, parse_file_list_entry_line,
    parse_rg_json_match_line, parse_search_match_line,
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

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
