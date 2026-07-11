use super::*;

#[test]
fn render_exact_appendix_skips_empty_ranges() {
    assert_eq!(
        smart_context_render_exact_appendix(
            "crit:",
            vec![SmartContextExactAppendixRange {
                reference: "psc:abcdef#L1-L1".to_string(),
                body: " \n ".to_string(),
            }],
        ),
        None
    );
}

#[test]
fn render_exact_appendix_merges_adjacent_and_overlapping_line_refs() {
    let (appendix, range_count) = smart_context_render_exact_appendix(
        "crit:",
        vec![
            SmartContextExactAppendixRange {
                reference: "psc:abcdef#L2-L4".to_string(),
                body: "beta\ngamma\ndelta".to_string(),
            },
            SmartContextExactAppendixRange {
                reference: "psc:abcdef#L4-L6".to_string(),
                body: "delta\nepsilon\nzeta".to_string(),
            },
        ],
    )
    .unwrap();

    assert_eq!(range_count, 2);
    assert_eq!(
        appendix,
        "crit:\npsc:abcdef#L2-L6\nbeta\ngamma\ndelta\nepsilon\nzeta"
    );
}

#[test]
fn render_exact_appendix_uses_duplicate_marker_with_compact_refs() {
    let duplicate_body = "same critical body with enough length for duplicate marker\n".repeat(4);
    let (appendix, range_count) = smart_context_render_exact_appendix(
        "crit:",
        vec![
            SmartContextExactAppendixRange {
                reference: "psc:abcdef#L1-L1".to_string(),
                body: duplicate_body.clone(),
            },
            SmartContextExactAppendixRange {
                reference: "psc:abcdef#L3-L3".to_string(),
                body: duplicate_body.clone(),
            },
            SmartContextExactAppendixRange {
                reference: "psc:abcdef#L5-L5".to_string(),
                body: duplicate_body.clone(),
            },
        ],
    )
    .unwrap();

    assert_eq!(range_count, 3);
    assert!(appendix.contains("[psc exdup h="));
    assert!(appendix.contains("refs=psc:abcdef#L1-L1,L3-L3]"));
    assert!(appendix.contains("psc:abcdef#L5-L5\n[psc exdup"));
}

#[test]
fn render_exact_appendix_preserves_uncompactable_duplicate_refs() {
    let duplicate_body = "same critical body with enough length for duplicate marker\n".repeat(4);
    let (appendix, range_count) = smart_context_render_exact_appendix(
        "crit:",
        vec![
            SmartContextExactAppendixRange {
                reference: "psc:custom-id#L1-L1".to_string(),
                body: duplicate_body.clone(),
            },
            SmartContextExactAppendixRange {
                reference: "psc:custom-id#L3-L3".to_string(),
                body: duplicate_body.clone(),
            },
            SmartContextExactAppendixRange {
                reference: "psc:custom-id#L5-L5".to_string(),
                body: duplicate_body.clone(),
            },
        ],
    )
    .unwrap();

    assert_eq!(range_count, 3);
    assert!(
        appendix.contains("refs=psc:custom-id#L1-L1,psc:custom-id#L3-L3]"),
        "{appendix}"
    );
    assert!(!appendix.contains("refs=psc:custom-id#L1-L1,L3-L3]"));
}

#[test]
fn artifact_line_ref_compaction_fuzzes_malformed_and_boundary_ranges() {
    for refs in [
        vec!["psc:sc:abcdef#L1-L1".to_string()],
        vec![
            "psc:sc:abcdef#L1-L1".to_string(),
            "psc:sc:abcdef#L2-L2".to_string(),
        ],
        vec![
            "psc:sc:abcdef#L0-L1".to_string(),
            "psc:sc:abcdef#L2-L1".to_string(),
        ],
        vec![
            "prodex-artifact:sc:abcdef#lines=L1-L3,L4-L4".to_string(),
            "psc:sc:abcdef#L5-L6".to_string(),
        ],
        vec![
            "psc:not-scoped#L1-L1".to_string(),
            "psc:not-scoped#L2-L2".to_string(),
        ],
        vec!["psc:sc:abcdef#L999999-L1000000".to_string()],
        vec!["psc:sc:abcdef#not-a-range".to_string()],
        vec!["plain text without a range".to_string()],
    ] {
        let compacted = smart_context_compact_line_refs_if_shorter(&refs);
        assert!(!compacted.is_empty());
        for reference in &refs {
            assert!(
                compacted.contains(reference)
                    || compacted.contains("psc:sc:abcdef#")
                    || compacted.contains("prodex-artifact:sc:abcdef#"),
                "compaction lost traceability: refs={refs:?} compacted={compacted}"
            );
        }
    }
}
