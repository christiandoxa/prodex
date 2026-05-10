use super::*;

#[test]
fn recall_content_dedupe_keeps_required_copy_and_replaces_optional_exact_duplicates() {
    let content = "critical memory line\n".repeat(30);
    let entries = runtime_mem_dedupe_recall_content([
        RuntimeMemRecallDedupeItem {
            id: "required-main".to_string(),
            content: content.clone(),
            required: true,
            artifact_ref: None,
        },
        RuntimeMemRecallDedupeItem {
            id: "optional-dup".to_string(),
            content: content.clone(),
            required: false,
            artifact_ref: None,
        },
    ]);

    assert_eq!(entries[0].replacement, None);
    assert_eq!(entries[0].content, content);
    assert_eq!(
        entries[1].reason,
        Some(RuntimeMemRecallDedupeReason::Duplicate {
            original_id: "required-main".to_string()
        })
    );
    let replacement = entries[1]
        .replacement
        .as_deref()
        .expect("optional duplicate should be replaced");
    assert!(replacement.contains("mem dup: original=required-main"));
    assert!(replacement.contains("h=sc:"));
    assert!(replacement.contains("b="));
    assert!(
        !replacement.contains("critical memory line"),
        "duplicate replacement must not semantic-summarize content"
    );
}

#[test]
fn recall_content_dedupe_replaces_optional_prodex_artifact_content_with_ref() {
    let content = "large artifact-backed memory\n".repeat(40);
    let entries = runtime_mem_dedupe_recall_content([
        RuntimeMemRecallDedupeItem {
            id: "artifact-backed".to_string(),
            content: content.clone(),
            required: false,
            artifact_ref: Some("prodex-artifact:sc:abc123".to_string()),
        },
        RuntimeMemRecallDedupeItem {
            id: "required-artifact-backed".to_string(),
            content,
            required: true,
            artifact_ref: Some("prodex-artifact:sc:def456".to_string()),
        },
    ]);

    assert_eq!(
        entries[0].reason,
        Some(RuntimeMemRecallDedupeReason::ArtifactRef {
            artifact_ref: "prodex-artifact:sc:abc123".to_string()
        })
    );
    assert_eq!(entries[1].replacement, None);
    let replacement = entries[0]
        .replacement
        .as_deref()
        .expect("artifact-backed optional content should be replaced");
    assert!(replacement.starts_with("prodex-artifact:sc:abc123"));
    assert!(replacement.contains("[mem art;"));
    assert!(replacement.contains("h=sc:"));
    assert!(replacement.contains("b="));
    assert!(
        !replacement.contains("large artifact-backed memory"),
        "artifact replacement must stay reference-only"
    );
}

#[test]
fn prodex_artifact_ref_helpers_accept_short_and_alias_refs() {
    assert_eq!(
        runtime_mem_normalize_prodex_artifact_ref("psc:0123456789abcdef"),
        Some("psc:0123456789abcdef".to_string())
    );
    assert_eq!(
        runtime_mem_normalize_prodex_artifact_ref("p:0123456789abcdef"),
        Some("p:0123456789abcdef".to_string())
    );
    assert_eq!(
        runtime_mem_normalize_prodex_artifact_ref("prodex-artifact:sc:legacy"),
        Some("prodex-artifact:sc:legacy".to_string())
    );
    assert_eq!(
        runtime_mem_normalize_prodex_artifact_ref(
            "need @0 first\npsc aliases @0=psc:fedcba9876543210"
        ),
        Some("psc:fedcba9876543210".to_string())
    );

    let aliases_across_values =
        serde_json::json!(["need @0", "psc aliases @0=psc:0123456789abcdef"]);
    assert_eq!(
        runtime_mem_extract_artifact_marker(&aliases_across_values),
        Some("psc:0123456789abcdef".to_string())
    );
    assert!(runtime_mem_value_contains_artifact_marker(
        &serde_json::json!({
            "payload": {
                "message": "refer to psc:0123456789abcdef"
            }
        })
    ));
    assert!(runtime_mem_value_contains_artifact_marker(
        &serde_json::json!({
            "payload": {
                "message": "refer to p:0123456789abcdef"
            }
        })
    ));
    assert!(runtime_mem_value_contains_artifact_marker(
        &aliases_across_values
    ));
}
