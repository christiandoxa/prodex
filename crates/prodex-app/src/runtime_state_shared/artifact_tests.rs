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
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn runtime_smart_context_artifact_save_merges_existing_file() {
        let path = smart_context_artifact_temp_path("merge-save");
        remove_smart_context_artifact_temp_files(&path);

        let mut first = RuntimeSmartContextArtifactStore::default();
        let alpha = first.insert_text(1, "alpha").expect("alpha artifact");
        first.save_to_path(&path).expect("first store saved");

        let mut second = RuntimeSmartContextArtifactStore::default();
        let beta = second.insert_text(2, "beta").expect("beta artifact");
        second.save_to_path(&path).expect("second store saved");

        let loaded = RuntimeSmartContextArtifactStore::load_from_path(&path);
        assert_eq!(loaded.artifact_count(), 2);
        assert_eq!(loaded.get_text(&alpha.id).as_deref(), Some("alpha"));
        assert_eq!(loaded.get_text(&beta.id).as_deref(), Some("beta"));

        remove_smart_context_artifact_temp_files(&path);
    }

    #[cfg(unix)]
    #[test]
    fn runtime_smart_context_artifact_save_replaces_symlink_without_reading_target() {
        let path = smart_context_artifact_temp_path("symlink-save");
        remove_smart_context_artifact_temp_files(&path);
        let target = path.with_file_name("outside-artifacts.json");
        fs::write(&target, r#"{"artifacts":{},"total_bytes":0}"#).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();

        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store.insert_text(1, "safe artifact").unwrap();
        store.save_to_path(&path).expect("store saved");

        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            r#"{"artifacts":{},"total_bytes":0}"#
        );
        assert!(
            !fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let loaded = RuntimeSmartContextArtifactStore::load_from_path(&path);
        assert_eq!(loaded.artifact_count(), 1);
        assert_eq!(
            loaded.get_text(&artifact.id).as_deref(),
            Some("safe artifact")
        );

        remove_smart_context_artifact_temp_files(&path);
        let _ = fs::remove_file(target);
    }

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
    fn runtime_smart_context_artifact_save_persists_static_fingerprints() {
        let path = smart_context_artifact_temp_path("static-fingerprints");
        remove_smart_context_artifact_temp_files(&path);

        let mut store = RuntimeSmartContextArtifactStore::default();
        store.set_static_context_fingerprints(
            Some("scpc:1234".to_string()),
            vec![runtime_proxy_crate::SmartContextFingerprint {
                id: "instructions".to_string(),
                kind: runtime_proxy_crate::SmartContextFingerprintKind::StaticContext,
                content_hash: "hash-a".to_string(),
                byte_len: 42,
            }],
        );
        store.save_to_path(&path).expect("store saved");

        let loaded = RuntimeSmartContextArtifactStore::load_from_path(&path);
        assert_eq!(loaded.static_context_prompt_cache_hash(), Some("scpc:1234"));
        assert_eq!(loaded.static_context_fingerprints().len(), 1);
        assert_eq!(loaded.static_context_fingerprints()[0].id, "instructions");

        remove_smart_context_artifact_temp_files(&path);
    }

    #[test]
    fn runtime_smart_context_artifact_ref_for_exact_text_requires_exact_hash_match() {
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store
            .insert_text(1, "repeatable command output")
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
        let artifact = store.insert_text(1, text).expect("artifact inserted");

        let index = store
            .line_index(&artifact.id)
            .expect("new artifacts should carry a line index");
        assert!(index.complete);
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
        let artifact = store.insert_text(1, text).expect("artifact inserted");

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
        let artifact = store.insert_text(1, &text).expect("artifact inserted");

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
        store.insert_text(1, rust).expect("rust artifact inserted");
        store
            .insert_text(2, typescript)
            .expect("typescript artifact inserted");
        store
            .insert_text(3, python)
            .expect("python artifact inserted");

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
        store.insert_text(1, text).expect("artifact inserted");

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
        store.insert_text(1, &text).expect("artifact inserted");

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
    fn runtime_smart_context_artifact_save_prewarms_repo_and_symbol_maps() {
        let path = smart_context_artifact_temp_path("projection-prewarm");
        remove_smart_context_artifact_temp_files(&path);

        let text = "\
error[E0425]: missing value
 --> src/lib.rs:7:3
pub mod runtime {
}
fn launch_super() {
}";
        let mut store = RuntimeSmartContextArtifactStore::default();
        store.insert_text(1, text).expect("artifact inserted");
        store.save_to_path(&path).expect("store saved");

        let raw = std::fs::read_to_string(&path).expect("store json should exist");
        assert!(raw.contains("repo_map_prewarm"));
        assert!(raw.contains("symbol_map_prewarm"));

        let loaded = RuntimeSmartContextArtifactStore::load_from_path(&path);
        let repo_map = loaded.repo_map_projection(64);
        let symbol_map = loaded.symbol_map_projection(64);
        assert!(repo_map.entries.iter().any(|entry| {
            entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Path
                && entry.path.as_deref() == Some("src/lib.rs")
                && entry.range_start > 0
        }));
        assert!(symbol_map.entries.iter().any(|entry| {
            entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Module
                && entry.symbol.as_deref() == Some("runtime")
        }));
        assert!(symbol_map.entries.iter().any(|entry| {
            entry.kind == RuntimeSmartContextArtifactRepoMapEntryKind::Error
                && entry.code.as_deref() == Some("E0425")
        }));

        remove_smart_context_artifact_temp_files(&path);
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
        let artifact = store.insert_text(1, text).expect("artifact inserted");

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
        let artifact = store.insert_text(1, &text).expect("artifact inserted");

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
        let artifact = store.insert_text(1, &text).expect("artifact inserted");

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
        let artifact = store.insert_text(1, text).expect("artifact inserted");

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

    #[test]
    fn runtime_smart_context_artifact_json_without_line_index_still_loads() {
        let text = "error: old failure\nsrc/main.rs:22:5";
        let content_hash = runtime_proxy_crate::smart_context_hash_text(text);
        let mut artifacts = serde_json::Map::new();
        artifacts.insert(
            content_hash.clone(),
            serde_json::json!({
                "id": content_hash.clone(),
                "byte_len": text.len(),
                "content_hash": runtime_proxy_crate::smart_context_hash_text(text),
                "text": text,
                "sequence": 1
            }),
        );
        let raw = serde_json::json!({
            "artifacts": artifacts,
            "total_bytes": text.len()
        });

        let mut store: RuntimeSmartContextArtifactStore =
            serde_json::from_value(raw).expect("legacy artifact store should deserialize");

        assert_eq!(store.get_text(&content_hash).as_deref(), Some(text));
        assert!(store.line_index(&content_hash).is_none());
        assert!(store.chunk_index(&content_hash).is_none());

        store
            .insert_text(2, text)
            .expect("matching legacy artifact should refresh metadata");

        assert!(
            store
                .line_index(&content_hash)
                .is_some_and(|index| index.complete)
        );
        assert!(
            store
                .chunk_index(&content_hash)
                .is_some_and(|index| index.complete)
        );
        assert_eq!(store.get_text(&content_hash).as_deref(), Some(text));
    }

    #[test]
    fn runtime_smart_context_artifact_concurrent_saves_keep_all_artifacts() {
        let path = Arc::new(smart_context_artifact_temp_path("concurrent-merge-save"));
        remove_smart_context_artifact_temp_files(path.as_ref());
        let thread_count = 8;
        let barrier = Arc::new(Barrier::new(thread_count));

        let handles = (0..thread_count)
            .map(|index| {
                let path = Arc::clone(&path);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    let text = format!("artifact-{index}");
                    let mut store = RuntimeSmartContextArtifactStore::default();
                    let artifact = store
                        .insert_text(index as u64 + 1, &text)
                        .expect("artifact inserted");
                    barrier.wait();
                    store.save_to_path(path.as_ref()).expect("store saved");
                    (artifact.id, text)
                })
            })
            .collect::<Vec<_>>();

        let expected = handles
            .into_iter()
            .map(|handle| handle.join().expect("save thread joined"))
            .collect::<Vec<_>>();

        let loaded = RuntimeSmartContextArtifactStore::load_from_path(path.as_ref());
        assert_eq!(loaded.artifact_count(), thread_count);
        for (id, text) in expected {
            assert_eq!(loaded.get_text(&id).as_deref(), Some(text.as_str()));
        }

        remove_smart_context_artifact_temp_files(path.as_ref());
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
