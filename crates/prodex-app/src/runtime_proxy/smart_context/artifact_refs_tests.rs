use super::*;

#[test]
fn compiled_artifact_projection_keeps_first_alias_and_protects_static_context() {
    let first = "sc:0123456789abcdef";
    let second = "sc:fedcba9876543210";
    let value = serde_json::json!({
        "a": "@2=psc:0123456789abcdef @2=psc:fedcba9876543210",
        "input": [
            {"role": "system", "content": "@3=psc:fedcba9876543210 @2"},
            {"role": "user", "content": "東京🧭 (@2#L2-L3,1.) @3 @4"},
            {"content": first},
            {"developer": {"nested": "@2"}},
            {"role": "SYSTEM", "content": "psc:0123456789abcdef"},
        ],
        "instructions": "@2 psc:fedcba9876543210",
    });
    assert_eq!(
        runtime_smart_context_collect_artifact_aliases(&value),
        BTreeMap::from([
            ("@2".to_string(), first.to_string()),
            ("@3".to_string(), second.to_string()),
        ])
    );
    let range = RuntimeSmartContextLineRange { start: 2, end: 3 };
    assert_eq!(
        runtime_smart_context_collect_rehydratable_artifact_refs(&value),
        vec![
            RuntimeSmartContextArtifactReference {
                id: first.to_string(),
                marker: "@2#L2-L3,1".to_string(),
                line_range: Some(range),
                line_ranges: vec![range, RuntimeSmartContextLineRange { start: 1, end: 1 }],
            },
            RuntimeSmartContextArtifactReference {
                id: second.to_string(),
                marker: "@3".to_string(),
                line_range: None,
                line_ranges: vec![],
            },
            RuntimeSmartContextArtifactReference {
                id: first.to_string(),
                marker: "psc:0123456789abcdef".to_string(),
                line_range: None,
                line_ranges: vec![],
            },
        ]
    );
    assert_eq!(
        runtime_smart_context_collect_rehydratable_artifact_ref_ids(&value),
        [first, second]
    );
}

#[test]
fn compiled_artifact_projection_resolves_punctuation_unicode_and_multiple_ranges() {
    let id = format!("sc2:{}", "b".repeat(64));
    let aliases = BTreeMap::from([("@12".to_string(), id.clone())]);
    let references = runtime_smart_context_artifact_ref_occurrences_from_text(
        &format!(
            "\u{2003}[@12#L9-L10,2-L4,0-2,8-7,999999999999999999999999] \"{id}\" @99 @12=psc:0123456789abcdef"
        ),
        &aliases,
    );
    assert_eq!(references.len(), 2);
    assert_eq!(references[0].id, id);
    assert_eq!(
        references[0].line_ranges,
        [
            RuntimeSmartContextLineRange { start: 9, end: 10 },
            RuntimeSmartContextLineRange { start: 2, end: 4 },
        ]
    );
    assert_eq!(references[1].id, id);
    assert!(references[1].line_ranges.is_empty());
}

#[test]
fn compiled_alias_ordering_keeps_first_of_many_repeated_declarations() {
    let first = "sc:0123456789abcdef";
    let declarations = (0..80)
        .map(|index| format!("@{}=psc:0123456789abcdef", index % 13))
        .chain((0..80).map(|index| format!("@{}=psc:fedcba9876543210", index % 13)))
        .collect::<Vec<_>>()
        .join(" ");
    let value = serde_json::json!({"input": [declarations, "@12 @1 @7"]});
    let aliases = runtime_smart_context_collect_artifact_aliases(&value);
    assert_eq!(aliases.len(), 13);
    assert!(aliases.values().all(|id| id == first));
    let references = runtime_smart_context_collect_rehydratable_artifact_refs(&value);
    assert_eq!(references.len(), 3);
    assert!(references.iter().all(|reference| reference.id == first));
}

#[test]
fn borrowed_artifact_acquisition_preserves_projection_without_serializing_text() {
    use prodex_mojo_core::json::{
        SmartContextArtifactProjection, smart_context_artifact_projection,
    };
    let value = serde_json::json!({"a": "@2=psc:0123456789abcdef", "input": ["東京\n\0@2"]});
    let wire = crate::json_boundary::app_json_tree(&value).unwrap();
    let borrowed = crate::json_boundary::app_json_tree_without_wire(&value).unwrap();
    assert!(borrowed.raw.is_empty());
    assert!(
        borrowed
            .nodes
            .iter()
            .all(|node| node.raw_start == 0 && node.raw_length == 0)
    );
    assert_eq!(
        smart_context_artifact_projection(
            &borrowed.nodes,
            "",
            SmartContextArtifactProjection::Request
        )
        .unwrap(),
        smart_context_artifact_projection(
            &wire.nodes,
            std::str::from_utf8(&wire.raw).unwrap(),
            SmartContextArtifactProjection::Request
        )
        .unwrap(),
    );
}
