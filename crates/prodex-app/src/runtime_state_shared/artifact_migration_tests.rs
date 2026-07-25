#[cfg(test)]
mod tests {
    use crate::runtime_state_shared::RuntimeSmartContextArtifactStore;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

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
            .insert_text(text)
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
    fn runtime_smart_context_artifact_store_migrates_legacy_fnv_identity() {
        let path = smart_context_artifact_temp_path("legacy-fnv-migration");
        remove_smart_context_artifact_temp_files(&path);
        let legacy_id = "sc:8ac625bb85ed202b";
        fs::write(
            &path,
            serde_json::json!({
                "schema_version": 1,
                "artifacts": {
                    (legacy_id): {
                        "id": legacy_id,
                        "byte_len": 5,
                        "content_hash": legacy_id,
                        "text": "alpha",
                        "sequence": 7
                    }
                },
                "total_bytes": 5
            })
            .to_string(),
        )
        .unwrap();

        let store = RuntimeSmartContextArtifactStore::load_from_path(&path);
        let strong_id = runtime_proxy_crate::smart_context_hash_text("alpha");

        assert_eq!(store.schema_version, 3);
        assert_eq!(store.get_text(&strong_id).as_deref(), Some("alpha"));
        assert_eq!(store.get_text(legacy_id).as_deref(), Some("alpha"));
        assert!(!store.artifacts.contains_key(legacy_id));
        remove_smart_context_artifact_temp_files(&path);
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
                    let artifact = store.insert_text(&text).expect("artifact inserted");
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
        let mut orders = loaded
            .artifacts
            .values()
            .map(|artifact| artifact.order)
            .collect::<Vec<_>>();
        orders.sort_unstable();
        assert_eq!(orders, (1..=thread_count as u64).collect::<Vec<_>>());
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
