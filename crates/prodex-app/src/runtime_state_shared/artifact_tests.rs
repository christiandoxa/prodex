#[cfg(test)]
mod tests {
    use crate::runtime_state_shared::{
        RUNTIME_SMART_CONTEXT_CHUNK_WINDOW_LINES, RUNTIME_SMART_CONTEXT_MAX_CHUNK_FINGERPRINTS,
        RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES,
        RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_LINE_INDEX_RANGES, RuntimeSmartContextArtifactLineIndex,
        RuntimeSmartContextArtifactRepoMapEntryKind, RuntimeSmartContextArtifactStore,
        runtime_smart_context_line_excerpt,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};
    #[test]
    fn runtime_smart_context_artifact_load_ignores_oversized_store_file() {
        let path = smart_context_artifact_temp_path("oversized-load");
        remove_smart_context_artifact_temp_files(&path);
        fs::File::create(&path)
            .unwrap()
            .set_len(64 * 1024 * 1024 + 1)
            .unwrap();

        let loaded = RuntimeSmartContextArtifactStore::load_from_path(&path);

        assert_eq!(loaded.artifact_count(), 0);
        remove_smart_context_artifact_temp_files(&path);
    }
    #[test]
    fn runtime_smart_context_artifact_ref_for_exact_text_requires_exact_hash_match() {
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store
            .insert_text("repeatable command output")
            .expect("artifact inserted");

        let found = store
            .artifact_ref_for_exact_text("repeatable command output")
            .expect("exact text should resolve to artifact ref");
        assert_eq!(found.id, artifact.id);
        assert_eq!(found.content_hash, artifact.content_hash);
        assert_eq!(found.byte_len, artifact.byte_len);
        assert!(
            store
                .artifact_ref_for_exact_text("repeatable command output with suffix")
                .is_none(),
            "near matches must not reuse artifacts"
        );
        store
            .artifacts
            .get_mut(&artifact.id)
            .expect("stored artifact")
            .content_hash = runtime_proxy_crate::smart_context_hash_text("stale content");
        assert!(
            store
                .artifact_ref_for_exact_text("repeatable command output")
                .is_none(),
            "stale artifact metadata must not produce an exact ref"
        );
    }

    #[test]
    fn runtime_smart_context_artifact_insert_stores_critical_line_index() {
        let text = "\
setup
error: hidden failure
src/main.rs:22:5
test result: FAILED. 0 passed; 1 failed
tail";
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store.insert_text(text).expect("artifact inserted");

        let index = store
            .line_index(&artifact.id)
            .expect("new artifacts should carry a line index");
        assert_eq!(index.complete, prodex_context::critical_signal_available());
        if prodex_context::critical_signal_available() {
            assert!(
                index
                    .critical_ranges
                    .iter()
                    .any(|range| range.text.contains("error: hidden failure"))
            );
            assert!(
                index
                    .critical_ranges
                    .iter()
                    .any(|range| range.text.contains("src/main.rs:22:5"))
            );
            assert!(
                index
                    .critical_ranges
                    .iter()
                    .any(|range| range.text.contains("test result: FAILED"))
            );
        } else {
            assert!(index.critical_ranges.is_empty());
        }
        for range in &index.critical_ranges {
            assert_eq!(range.byte_len, range.text.len());
            assert_eq!(
                range.content_hash,
                runtime_proxy_crate::smart_context_hash_text(&range.text)
            );
        }
        assert_eq!(store.get_text(&artifact.id).as_deref(), Some(text));
    }

    #[test]
    fn runtime_smart_context_artifact_insert_stores_semantic_line_index() {
        let text = "\
running 1 test
---- tests::keeps_failure_metadata stdout ----
thread 'tests::keeps_failure_metadata' panicked at src/main.rs:22:5:
error[E0277]: trait bound failed
 --> src/main.rs:22:5
--- a/src/main.rs
+++ b/src/main.rs
@@ -20,2 +20,3 @@ fn demo()
-old
+new
test result: FAILED. 0 passed; 1 failed";
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store.insert_text(text).expect("artifact inserted");

        let index = store
            .line_index(&artifact.id)
            .expect("new artifacts should carry a line index");
        assert_eq!(index.command_kind.as_deref(), Some("cargo-test"));
        assert!(
            index
                .file_location_ranges
                .iter()
                .any(|range| range.path.as_deref() == Some("src/main.rs")
                    && range.line == Some(22)
                    && range.column == Some(5))
        );
        assert!(index.diff_hunk_ranges.iter().any(|range| {
            range.path.as_deref() == Some("src/main.rs")
                && range.old_start == Some(20)
                && range.old_count == Some(2)
                && range.new_start == Some(20)
                && range.new_count == Some(3)
                && range.text.contains("+new")
        }));
        assert!(index.test_failure_ranges.iter().any(|range| {
            range.symbol.as_deref() == Some("tests::keeps_failure_metadata")
                || range.text.contains("test result: FAILED")
        }));
        assert!(
            index
                .error_ranges
                .iter()
                .any(|range| range.code.as_deref() == Some("E0277"))
        );
        for range in index
            .file_location_ranges
            .iter()
            .chain(index.diff_hunk_ranges.iter())
            .chain(index.test_failure_ranges.iter())
            .chain(index.error_ranges.iter())
        {
            assert_eq!(range.byte_len, range.text.len());
            assert_eq!(
                range.content_hash,
                runtime_proxy_crate::smart_context_hash_text(&range.text)
            );
            assert!(range.byte_len <= RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES);
        }
    }

    #[test]
    fn runtime_smart_context_artifact_semantic_line_index_is_bounded() {
        let text = (0..400)
            .map(|index| format!("src/file{index}.rs:{}:1: error[E0001]: failure", index + 1))
            .collect::<Vec<_>>()
            .join("\n");
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store.insert_text(&text).expect("artifact inserted");

        let index = store
            .line_index(&artifact.id)
            .expect("new artifacts should carry a line index");
        let semantic_range_count = index.file_location_ranges.len()
            + index.diff_hunk_ranges.len()
            + index.test_failure_ranges.len()
            + index.error_ranges.len();
        assert!(semantic_range_count <= RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_LINE_INDEX_RANGES);
        assert_eq!(
            semantic_range_count,
            RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_LINE_INDEX_RANGES
        );
        assert!(!index.semantic_complete);
    }

    #[test]
    fn runtime_smart_context_repo_map_projection_collects_multilanguage_symbols() {
        let rust = "\
src/lib.rs:1:1
pub mod engine {
}
#[test]
fn parses_repo_map() {
}
fn helper() {
}";
        let typescript = "\
src/ui/App.tsx:4:1
export class Widget {
}
test(\"renders widget\", () => {
});
const useWidget = () => {
};";
        let python = "\
tests/test_cli.py:1:1
class CliHarness:
    pass
def test_launch_super():
    pass
async def load_profile():
    pass";
        let mut store = RuntimeSmartContextArtifactStore::default();
        store.insert_text(rust).expect("rust artifact inserted");
        store
            .insert_text(typescript)
            .expect("typescript artifact inserted");
        store.insert_text(python).expect("python artifact inserted");

        let repo_map = store.repo_map_projection(64);

        assert!(repo_map.complete);
        assert!(repo_map.entries.iter().any(|entry| {
            entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Path
                && entry.path.as_deref() == Some("src/lib.rs")
                && entry.module.as_deref() == Some("lib")
        }));
        assert!(repo_map.entries.iter().any(|entry| {
            entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Module
                && entry.path.as_deref() == Some("src/lib.rs")
                && entry.symbol.as_deref() == Some("engine")
        }));
        assert!(repo_map.entries.iter().any(|entry| {
            entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Test
                && entry.symbol.as_deref() == Some("parses_repo_map")
        }));
        assert!(repo_map.entries.iter().any(|entry| {
            entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Symbol
                && entry.symbol.as_deref() == Some("helper")
                && entry.module.as_deref() == Some("lib")
        }));
        assert!(repo_map.entries.iter().any(|entry| {
            entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Module
                && entry.path.as_deref() == Some("src/ui/App.tsx")
                && entry.symbol.as_deref() == Some("Widget")
        }));
        assert!(repo_map.entries.iter().any(|entry| {
            entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Test
                && entry.path.as_deref() == Some("src/ui/App.tsx")
                && entry.symbol.as_deref() == Some("renders widget")
        }));
        assert!(repo_map.entries.iter().any(|entry| {
            entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Module
                && entry.path.as_deref() == Some("tests/test_cli.py")
                && entry.symbol.as_deref() == Some("CliHarness")
        }));
        assert!(repo_map.entries.iter().any(|entry| {
            entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Test
                && entry.path.as_deref() == Some("tests/test_cli.py")
                && entry.symbol.as_deref() == Some("test_launch_super")
        }));
        assert!(repo_map.entries.iter().any(|entry| {
            entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Symbol
                && entry.path.as_deref() == Some("tests/test_cli.py")
                && entry.symbol.as_deref() == Some("load_profile")
        }));
    }

    #[test]
    fn runtime_smart_context_map_projections_include_error_entries_and_symbol_map() {
        let text = "\
error[E0425]: cannot find value `missing` in this scope
 --> src/lib.rs:7:3
fn target_symbol() {
}
#[test]
fn target_test() {
}";
        let mut store = RuntimeSmartContextArtifactStore::default();
        store.insert_text(text).expect("artifact inserted");

        let repo_map = store.repo_map_projection(64);
        assert!(repo_map.entries.iter().any(|entry| {
            entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Error
                && entry.code.as_deref() == Some("E0425")
                && entry.path.as_deref() == Some("src/lib.rs")
                && entry.range_start == 1
        }));

        let symbol_map = store.symbol_map_projection(64);
        assert!(symbol_map.complete);
        assert!(
            symbol_map
                .entries
                .iter()
                .all(|entry| entry.kind != RuntimeSmartContextArtifactRepoMapEntryKind::Path)
        );
        assert!(symbol_map.entries.iter().any(|entry| {
            entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Symbol
                && entry.symbol.as_deref() == Some("target_symbol")
                && entry.path.as_deref() == Some("src/lib.rs")
        }));
        assert!(symbol_map.entries.iter().any(|entry| {
            entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Test
                && entry.symbol.as_deref() == Some("target_test")
                && entry.path.as_deref() == Some("src/lib.rs")
        }));
        assert!(symbol_map.entries.iter().any(|entry| {
            entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Error
                && entry.code.as_deref() == Some("E0425")
        }));
    }

    #[test]
    fn runtime_smart_context_repo_map_projection_is_bounded_and_deterministic() {
        let text = (0..80)
            .flat_map(|index| {
                [
                    format!("src/file{index}.rs:1:1"),
                    format!("fn symbol_{index}() {{}}"),
                ]
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut store = RuntimeSmartContextArtifactStore::default();
        store.insert_text(&text).expect("artifact inserted");

        let first = store.repo_map_projection(16);
        let second = store.repo_map_projection(16);

        assert_eq!(first, second);
        assert_eq!(first.entries.len(), 16);
        assert!(!first.complete);
        assert!(
            first
                .entries
                .iter()
                .all(|entry| entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Path)
        );
    }
    #[test]
    fn runtime_smart_context_artifact_insert_stores_chunk_fingerprints() {
        let text = "\
running 1 test
---- tests::stores_chunk_fingerprints stdout ----
thread 'tests::stores_chunk_fingerprints' panicked at src/main.rs:22:5:
error[E0277]: trait bound failed
 --> src/main.rs:22:5
--- a/src/main.rs
+++ b/src/main.rs
@@ -20,2 +20,3 @@ fn demo()
-old
+new
test result: FAILED. 0 passed; 1 failed";
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store.insert_text(text).expect("artifact inserted");

        let chunk_index = store
            .chunk_index(&artifact.id)
            .expect("new artifacts should carry a chunk index");
        assert!(chunk_index.complete);
        assert!(
            chunk_index
                .chunks
                .iter()
                .any(|chunk| chunk.kind == "file" && chunk.path.as_deref() == Some("src/main.rs"))
        );
        assert!(
            chunk_index
                .chunks
                .iter()
                .any(|chunk| chunk.kind == "diff" && chunk.path.as_deref() == Some("src/main.rs"))
        );
        assert!(chunk_index.chunks.iter().any(|chunk| {
            chunk.kind == "test"
                && chunk
                    .symbol
                    .as_deref()
                    .is_some_and(|symbol| symbol.contains("stores_chunk_fingerprints"))
        }));
        assert!(
            chunk_index
                .chunks
                .iter()
                .any(|chunk| chunk.kind == "error" && chunk.code.as_deref() == Some("E0277"))
        );

        let lines = text.lines().collect::<Vec<_>>();
        for chunk in &chunk_index.chunks {
            let excerpt = runtime_smart_context_line_excerpt(&lines, chunk.start, chunk.end)
                .expect("chunk excerpt should resolve");
            assert_eq!(chunk.byte_len, excerpt.len());
            assert_eq!(
                chunk.content_hash,
                runtime_proxy_crate::smart_context_hash_text(&excerpt)
            );
            assert!(chunk.byte_len <= RUNTIME_SMART_CONTEXT_MAX_LINE_INDEX_EXCERPT_BYTES);
        }
    }

    #[test]
    fn runtime_smart_context_artifact_chunk_fingerprints_are_bounded() {
        let text = (0..400)
            .map(|index| format!("src/file{index}.rs:{}:1: error[E0001]: failure", index + 1))
            .collect::<Vec<_>>()
            .join("\n");
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store.insert_text(&text).expect("artifact inserted");

        let chunk_index = store
            .chunk_index(&artifact.id)
            .expect("new artifacts should carry a chunk index");
        assert!(chunk_index.chunks.len() <= RUNTIME_SMART_CONTEXT_MAX_CHUNK_FINGERPRINTS);
        assert_eq!(
            chunk_index.chunks.len(),
            RUNTIME_SMART_CONTEXT_MAX_CHUNK_FINGERPRINTS
        );
        assert!(!chunk_index.complete);
    }

    #[test]
    fn runtime_smart_context_artifact_chunk_fingerprints_fall_back_to_line_windows() {
        let text = (1..=70)
            .map(|line| format!("plain output line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store.insert_text(&text).expect("artifact inserted");

        let chunk_index = store
            .chunk_index(&artifact.id)
            .expect("new artifacts should carry a chunk index");
        assert!(chunk_index.complete);
        assert_eq!(chunk_index.chunks.len(), 3);
        assert!(
            chunk_index
                .chunks
                .iter()
                .all(|chunk| chunk.kind == "window")
        );

        let lines = text.lines().collect::<Vec<_>>();
        let first = chunk_index.chunks.first().expect("first window chunk");
        assert_eq!(first.start, 1);
        assert_eq!(first.end, RUNTIME_SMART_CONTEXT_CHUNK_WINDOW_LINES);
        let excerpt = runtime_smart_context_line_excerpt(&lines, first.start, first.end)
            .expect("first window excerpt should resolve");
        assert_eq!(
            first.content_hash,
            runtime_proxy_crate::smart_context_hash_text(&excerpt)
        );
    }

    #[test]
    fn runtime_smart_context_artifact_duplicate_chunk_metadata_is_recorded() {
        let text = "\
error[E0001]: repeated failure
ok
error[E0001]: repeated failure
ok
error[E0001]: repeated failure";
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store.insert_text(text).expect("artifact inserted");

        let chunk_index = store
            .chunk_index(&artifact.id)
            .expect("new artifacts should carry a chunk index");
        let repeated_hash =
            runtime_proxy_crate::smart_context_hash_text("error[E0001]: repeated failure");
        let duplicate = chunk_index
            .duplicate_chunks
            .iter()
            .find(|duplicate| duplicate.content_hash == repeated_hash)
            .expect("repeated semantic chunks should be summarized");
        assert_eq!(duplicate.occurrence_count, 3);
        assert_eq!(duplicate.byte_len, "error[E0001]: repeated failure".len());
        assert_eq!(duplicate.occurrences.len(), 3);
        assert!(
            duplicate
                .occurrences
                .iter()
                .all(|occurrence| occurrence.kind == "error")
        );
        assert_eq!(
            duplicate
                .occurrences
                .iter()
                .map(|occurrence| occurrence.start)
                .collect::<Vec<_>>(),
            vec![1, 3, 5]
        );
    }

    #[test]
    fn runtime_smart_context_artifact_line_index_json_without_semantic_fields_still_loads() {
        let raw = serde_json::json!({
            "complete": true,
            "critical_ranges": [{
                "start": 1,
                "end": 1,
                "byte_len": 12,
                "content_hash": runtime_proxy_crate::smart_context_hash_text("error: old"),
                "text": "error: old"
            }]
        });

        let index: RuntimeSmartContextArtifactLineIndex =
            serde_json::from_value(raw).expect("legacy line index should deserialize");

        assert!(index.complete);
        assert!(index.semantic_complete);
        assert_eq!(index.critical_ranges.len(), 1);
        assert!(index.file_location_ranges.is_empty());
        assert!(index.diff_hunk_ranges.is_empty());
        assert!(index.test_failure_ranges.is_empty());
        assert!(index.error_ranges.is_empty());
        assert!(index.command_kind.is_none());

        let serialized = serde_json::to_value(&index).expect("line index should serialize");
        assert!(serialized.get("file_location_ranges").is_none());
        assert!(serialized.get("diff_hunk_ranges").is_none());
        assert!(serialized.get("test_failure_ranges").is_none());
        assert!(serialized.get("error_ranges").is_none());
        assert!(serialized.get("command_kind").is_none());
    }

    fn smart_context_artifact_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "prodex-app-smart-context-artifacts-{name}-{}-{nanos}.json",
            std::process::id()
        ))
    }

    fn remove_smart_context_artifact_temp_files(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(crate::runtime_store::json_lock_file_path(path));
    }
}
