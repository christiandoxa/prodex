use super::{
    RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_LINE_INDEX_RANGES,
    RuntimeSmartContextArtifactSemanticLineRange,
};

#[path = "semantic_index/markers.rs"]
mod markers;
#[path = "semantic_index/ranges.rs"]
mod ranges;
#[path = "semantic_index/types.rs"]
mod types;
#[path = "semantic_index/util.rs"]
mod util;

pub(super) use ranges::*;
pub(super) use types::*;
pub(super) use util::*;

pub(super) fn runtime_smart_context_artifact_semantic_line_index(
    text: &str,
    lines: &[&str],
) -> RuntimeSmartContextArtifactSemanticLineIndexParts {
    let semantic = prodex_mojo_core::smart_context_markers::semantic_index(
        text,
        lines,
        RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_LINE_INDEX_RANGES,
        crate::runtime_state_shared::RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES,
    )
    .expect("Mojo Smart Context semantic index returned invalid output");
    let remaining =
        RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_LINE_INDEX_RANGES.saturating_sub(semantic.ranges.len());
    let mut parts = RuntimeSmartContextArtifactSemanticLineIndexParts {
        complete: semantic.complete,
        symbol_complete: true,
        command_kind: semantic.command_kind.map(|kind| {
            match kind {
                prodex_mojo_core::smart_context_markers::CommandLineKind::Python => "python",
                prodex_mojo_core::smart_context_markers::CommandLineKind::Diff => "diff",
                prodex_mojo_core::smart_context_markers::CommandLineKind::CargoTest => "cargo-test",
                prodex_mojo_core::smart_context_markers::CommandLineKind::CargoBuild => {
                    "cargo-build"
                }
                prodex_mojo_core::smart_context_markers::CommandLineKind::NpmTest => "npm-test",
            }
            .to_string()
        }),
        ..Default::default()
    };

    for plan in semantic.ranges {
        let range = runtime_smart_context_materialize_semantic_range(lines, &plan);
        match plan.kind {
            prodex_mojo_core::smart_context_markers::SemanticRangeKind::FileLocation => {
                parts.file_location_ranges.push(range)
            }
            prodex_mojo_core::smart_context_markers::SemanticRangeKind::DiffHunk => {
                parts.diff_hunk_ranges.push(range)
            }
            prodex_mojo_core::smart_context_markers::SemanticRangeKind::TestFailure => {
                parts.test_failure_ranges.push(range)
            }
            prodex_mojo_core::smart_context_markers::SemanticRangeKind::Error => {
                parts.error_ranges.push(range)
            }
        }
    }

    let symbols = prodex_mojo_core::smart_context_symbols::index(
        text,
        remaining,
        crate::runtime_state_shared::RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES,
    )
    .expect("Mojo Smart Context symbol index returned invalid output");
    parts.symbol_complete = symbols.complete;
    for symbol in symbols.ranges {
        let text = lines[symbol.start_line - 1..symbol.end_line].join("\n");
        parts
            .symbol_ranges
            .push(RuntimeSmartContextArtifactSemanticLineRange {
                start: symbol.start_line,
                end: symbol.end_line,
                byte_len: text.len(),
                content_hash: runtime_proxy_crate::smart_context_hash_text(&text),
                text,
                label: Some(
                    match symbol.label {
                        prodex_mojo_core::smart_context_symbols::SymbolLabel::Function => {
                            "function"
                        }
                        prodex_mojo_core::smart_context_symbols::SymbolLabel::Test => "test_symbol",
                        prodex_mojo_core::smart_context_symbols::SymbolLabel::Symbol => "symbol",
                    }
                    .to_string(),
                ),
                line: Some(symbol.declaration_line),
                symbol: Some(symbol.symbol),
                path: None,
                column: None,
                old_start: None,
                old_count: None,
                new_start: None,
                new_count: None,
                code: None,
            });
    }

    parts
}
