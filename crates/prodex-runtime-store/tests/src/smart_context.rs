use super::*;
use std::path::PathBuf;

#[test]
fn smart_context_stale_pruning_reuses_exact_large_payload() {
    let decision = runtime_smart_context_stale_context_pruning_decision(
        RuntimeSmartContextStaleContextPruningInput {
            previous: Some(RuntimeSmartContextStaleContextSnapshot {
                hash: Some("fnv1a64:aaaaaaaaaaaaaaaa"),
                byte_len: 8_192,
                token_len: 2_048,
            }),
            current: RuntimeSmartContextStaleContextSnapshot {
                hash: Some("fnv1a64:aaaaaaaaaaaaaaaa"),
                byte_len: 8_192,
                token_len: 2_048,
            },
            changed: true,
        },
    );

    assert_eq!(
        decision.kind,
        RuntimeSmartContextStaleContextPruningKind::ExactReuse
    );
    assert!(decision.kind.can_prune_payload());
    assert_eq!(decision.reusable_byte_len, 8_192);
    assert_eq!(decision.reusable_token_len, 2_048);
    assert_eq!(
        decision.summary,
        "static-context: reuse hash=fnv1a64:aaaaaaaaaaaaaaaa bytes=8192 tokens=2048 saved_bytes=8192 saved_tokens=2048"
    );
}

#[test]
fn smart_context_stale_pruning_reports_changed_large_payload_delta() {
    let decision = runtime_smart_context_stale_context_pruning_decision(
        RuntimeSmartContextStaleContextPruningInput {
            previous: Some(RuntimeSmartContextStaleContextSnapshot {
                hash: Some("fnv1a64:aaaaaaaaaaaaaaaa"),
                byte_len: 8_192,
                token_len: 2_048,
            }),
            current: RuntimeSmartContextStaleContextSnapshot {
                hash: Some("fnv1a64:bbbbbbbbbbbbbbbb"),
                byte_len: 9_000,
                token_len: 1_900,
            },
            changed: false,
        },
    );

    assert_eq!(
        decision.kind,
        RuntimeSmartContextStaleContextPruningKind::Changed
    );
    assert!(decision.kind.can_prune_payload());
    assert_eq!(decision.reusable_byte_len, 0);
    assert_eq!(
        decision.previous_hash.as_deref(),
        Some("fnv1a64:aaaaaaaaaaaaaaaa")
    );
    assert_eq!(
        decision.current_hash.as_deref(),
        Some("fnv1a64:bbbbbbbbbbbbbbbb")
    );
    assert_eq!(
        decision.summary,
        "static-context: changed previous_hash=fnv1a64:aaaaaaaaaaaaaaaa current_hash=fnv1a64:bbbbbbbbbbbbbbbb previous_bytes=8192 current_bytes=9000 previous_tokens=2048 current_tokens=1900 byte_delta=+808 token_delta=-148"
    );
}

#[test]
fn smart_context_stale_pruning_no_previous_is_noop() {
    let decision = runtime_smart_context_stale_context_pruning_decision(
        RuntimeSmartContextStaleContextPruningInput {
            previous: None,
            current: RuntimeSmartContextStaleContextSnapshot {
                hash: Some(" fnv1a64:cccccccccccccccc "),
                byte_len: 4_096,
                token_len: 1_024,
            },
            changed: true,
        },
    );

    assert_eq!(
        decision.kind,
        RuntimeSmartContextStaleContextPruningKind::NoPrevious
    );
    assert!(!decision.kind.can_prune_payload());
    assert_eq!(
        decision.current_hash.as_deref(),
        Some("fnv1a64:cccccccccccccccc")
    );
    assert_eq!(
        decision.summary,
        "static-context: no-op reason=no_previous current_hash=fnv1a64:cccccccccccccccc current_bytes=4096 current_tokens=1024"
    );
}

#[test]
fn smart_context_stale_pruning_too_small_is_noop_before_reuse() {
    let decision = runtime_smart_context_stale_context_pruning_decision(
        RuntimeSmartContextStaleContextPruningInput {
            previous: Some(RuntimeSmartContextStaleContextSnapshot {
                hash: Some("fnv1a64:dddddddddddddddd"),
                byte_len: 128,
                token_len: 32,
            }),
            current: RuntimeSmartContextStaleContextSnapshot {
                hash: Some("fnv1a64:dddddddddddddddd"),
                byte_len: 128,
                token_len: 32,
            },
            changed: false,
        },
    );

    assert_eq!(
        decision.kind,
        RuntimeSmartContextStaleContextPruningKind::TooSmall
    );
    assert!(!decision.kind.can_prune_payload());
    assert_eq!(decision.reusable_byte_len, 0);
    assert_eq!(
        decision.summary,
        "static-context: no-op reason=too_small current_hash=fnv1a64:dddddddddddddddd current_bytes=128 current_tokens=32"
    );
}

#[test]
fn smart_context_artifact_helpers_hash_touch_and_extract_line_ranges() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let content = "one\ntwo\nthree\n";
    let artifact =
        runtime_smart_context_upsert_artifact(&mut store, "file:src/lib.rs", content, 100);

    assert_eq!(artifact.byte_len, content.len());
    assert_eq!(
        artifact.content_hash,
        runtime_smart_context_artifact_content_hash(content.as_bytes())
    );

    let artifact =
        runtime_smart_context_upsert_artifact(&mut store, "file:src/lib.rs", content, 120);
    assert_eq!(artifact.created_at, 100);
    assert_eq!(artifact.last_accessed_at, 120);

    let touched = runtime_smart_context_touch_artifact(&mut store, "file:src/lib.rs", 130)
        .expect("artifact touched");
    assert_eq!(touched.last_accessed_at, 130);

    let extracted = runtime_smart_context_artifact_line_range(
        touched,
        RuntimeSmartContextLineRange {
            start_line: 2,
            end_line: 3,
        },
    )
    .expect("line range extracted");
    assert_eq!(extracted.start_line, 2);
    assert_eq!(extracted.end_line, 3);
    assert_eq!(extracted.content, "two\nthree\n");

    assert!(
        runtime_smart_context_extract_line_range(
            content,
            RuntimeSmartContextLineRange {
                start_line: 4,
                end_line: 4,
            },
        )
        .is_none()
    );
}

#[test]
fn smart_context_artifact_compaction_applies_ttl_and_max_entries() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    for (key, created_at, last_accessed_at) in [
        ("expired", 70, 80),
        ("cold", 90, 95),
        ("warm", 91, 97),
        ("hot", 92, 99),
    ] {
        let mut artifact = runtime_smart_context_artifact_from_content(key, key, created_at);
        artifact.last_accessed_at = last_accessed_at;
        store.artifacts.insert(key.to_string(), artifact);
    }

    let compacted = compact_runtime_smart_context_artifact_store(
        store,
        100,
        RuntimeSmartContextArtifactStorePolicy {
            ttl_seconds: 10,
            max_entries: 2,
        },
    );

    assert_eq!(
        compacted.artifacts.keys().cloned().collect::<Vec<_>>(),
        vec!["hot".to_string(), "warm".to_string()]
    );
}

#[test]
fn smart_context_artifact_json_round_trips_and_validates_metadata() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    runtime_smart_context_upsert_artifact(
        &mut store,
        "path:\"quoted\"\\name",
        "line \"one\"\nslash \\ tab\tend",
        100,
    );

    let json = runtime_smart_context_artifact_store_to_json(&store);
    assert!(json.contains("\\\"quoted\\\""));
    assert!(json.contains("\\n"));
    assert!(json.contains("\\t"));

    let parsed = runtime_smart_context_artifact_store_from_json(&json).expect("store json parsed");
    assert_eq!(parsed, store);

    let hash = store
        .artifacts
        .values()
        .next()
        .expect("artifact")
        .content_hash
        .clone();
    let bad_json = json.replace(&hash, "fnv1a64:0000000000000000");
    assert!(
        runtime_smart_context_artifact_store_from_json(&bad_json)
            .expect_err("bad hash rejected")
            .message
            .contains("content_hash mismatch")
    );
}

#[test]
fn smart_context_artifact_file_save_merges_existing_json() {
    let path = smart_context_temp_path("merge");
    let _ = std::fs::remove_file(&path);
    let policy = RuntimeSmartContextArtifactStorePolicy {
        ttl_seconds: 100,
        max_entries: 8,
    };

    let mut existing = RuntimeSmartContextArtifactStore::default();
    runtime_smart_context_upsert_artifact(&mut existing, "a", "alpha", 10);
    save_runtime_smart_context_artifact_store(&path, &existing, 20, policy)
        .expect("existing store saved");

    let mut incoming = RuntimeSmartContextArtifactStore::default();
    runtime_smart_context_upsert_artifact(&mut incoming, "a", "alpha", 40);
    runtime_smart_context_upsert_artifact(&mut incoming, "b", "beta", 40);
    let merged = save_merged_runtime_smart_context_artifact_store(&path, &incoming, 40, policy)
        .expect("merged store saved");

    assert_eq!(
        merged.artifacts.keys().cloned().collect::<Vec<_>>(),
        vec!["a".to_string(), "b".to_string()]
    );
    let a = merged.artifacts.get("a").expect("merged a");
    assert_eq!(a.created_at, 10);
    assert_eq!(a.last_accessed_at, 40);

    let loaded =
        load_runtime_smart_context_artifact_store(&path, 40, policy).expect("store loaded");
    assert_eq!(loaded, merged);
    std::fs::remove_file(path).expect("temp store removed");
}

fn smart_context_temp_path(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "prodex-runtime-store-smart-context-{name}-{}-{nanos}.json",
        std::process::id()
    ))
}
