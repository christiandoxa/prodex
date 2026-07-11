use crate::runtime_state_shared::RuntimeSmartContextArtifactSemanticLineRange;

#[derive(Default)]
pub(in crate::runtime_state_shared) struct RuntimeSmartContextArtifactSemanticLineIndexParts {
    pub(in crate::runtime_state_shared) file_location_ranges:
        Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    pub(in crate::runtime_state_shared) diff_hunk_ranges:
        Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    pub(in crate::runtime_state_shared) test_failure_ranges:
        Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    pub(in crate::runtime_state_shared) error_ranges:
        Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    pub(in crate::runtime_state_shared) symbol_ranges:
        Vec<RuntimeSmartContextArtifactSemanticLineRange>,
    pub(in crate::runtime_state_shared) complete: bool,
    pub(in crate::runtime_state_shared) symbol_complete: bool,
}

#[derive(Debug, Clone)]
pub(in crate::runtime_state_shared) struct RuntimeSmartContextParsedFileLocation {
    pub(super) path: String,
    pub(super) line: usize,
    pub(super) column: Option<usize>,
}

#[derive(Debug, Clone)]
pub(in crate::runtime_state_shared) struct RuntimeSmartContextParsedDiffHunk {
    pub(super) old_start: usize,
    pub(super) old_count: usize,
    pub(super) new_start: usize,
    pub(super) new_count: usize,
}

#[derive(Default)]
pub(in crate::runtime_state_shared) struct RuntimeSmartContextSemanticRangeMetadata {
    pub(super) label: Option<String>,
    pub(super) path: Option<String>,
    pub(super) line: Option<usize>,
    pub(super) column: Option<usize>,
    pub(super) old_start: Option<usize>,
    pub(super) old_count: Option<usize>,
    pub(super) new_start: Option<usize>,
    pub(super) new_count: Option<usize>,
    pub(super) code: Option<String>,
    pub(super) symbol: Option<String>,
}

#[derive(Debug, Clone)]
pub(in crate::runtime_state_shared) struct RuntimeSmartContextParsedSymbolLine {
    pub(super) label: &'static str,
    pub(super) symbol: String,
    pub(super) style: RuntimeSmartContextSymbolRangeStyle,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::runtime_state_shared) enum RuntimeSmartContextSymbolRangeStyle {
    Brace,
    Python,
}
