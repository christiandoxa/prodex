use super::*;
#[test]
fn rich_domain_self_test_covers_structured_results() {
    assert!(rich_self_test());
}

#[test]
fn rich_abi_rejects_null_views_and_reports_utf8_offsets() {
    ensure_rich_abi().expect("rich ABI layout should match");
    let mut result = RichContextResult::default();
    let mut records = [RichContextRecord::default()];
    let mut slots = [-1_i64];
    let invalid = [0xff_u8];
    let invalid_view = RichStringView {
        ptr: mojo_pointer_address(invalid.as_ptr()),
        len: invalid.len() as u64,
    };
    let status = unsafe {
        prodex_mojo_rich_context_analyze_v2(
            RICH_ABI_VERSION,
            mojo_pointer_address(&invalid_view),
            mojo_pointer_address(records.as_mut_ptr()),
            records.len() as i64,
            0,
            0,
            mojo_pointer_address(slots.as_mut_ptr()),
            slots.len() as i64,
            mojo_mut_pointer_address(&mut result),
        )
    };
    assert_eq!(status, RICH_STATUS_UTF8);
    assert_eq!(result.issue_kind, 6);
    assert_eq!(result.issue_offset, 0);

    let empty_view = RichStringView::default();
    let status = unsafe {
        prodex_mojo_rich_context_analyze_v2(
            RICH_ABI_VERSION,
            mojo_pointer_address(&empty_view),
            0,
            0,
            0,
            0,
            mojo_pointer_address(slots.as_mut_ptr()),
            slots.len() as i64,
            0,
        )
    };
    assert_eq!(status, RICH_STATUS_INVALID);
}

#[test]
fn rich_abi_malformed_utf8_and_capacity_are_bounded() {
    ensure_rich_abi().expect("rich ABI layout should match");
    let malformed = [
        vec![0xff],
        vec![0xc0, 0x80],
        vec![0xe0, 0x80, 0x80],
        vec![0xed, 0xa0, 0x80],
        vec![0xf0, 0x80, 0x80, 0x80],
        vec![0xf4, 0x90, 0x80, 0x80],
        vec![0xf0, 0x90, 0x80],
    ];
    for case in 0..20_000 {
        let bytes = &malformed[case % malformed.len()];
        let mut result = RichContextResult::default();
        let mut records = [RichContextRecord::default()];
        let mut output = [0_u8; 8];
        let mut slots = [-1_i64; 2];
        let input_view = RichStringView {
            ptr: mojo_pointer_address(bytes.as_ptr()),
            len: bytes.len() as u64,
        };
        let status = unsafe {
            prodex_mojo_rich_context_analyze_v2(
                RICH_ABI_VERSION,
                mojo_pointer_address(&input_view),
                mojo_pointer_address(records.as_mut_ptr()),
                records.len() as i64,
                mojo_pointer_address(output.as_mut_ptr()),
                output.len() as i64,
                mojo_pointer_address(slots.as_mut_ptr()),
                slots.len() as i64,
                mojo_mut_pointer_address(&mut result),
            )
        };
        assert_eq!(status, RICH_STATUS_UTF8, "malformed case {case}");
        assert_eq!(result.issue_kind, 6, "malformed case {case}");
        assert!(result.issue_offset >= 0, "malformed case {case}");
    }

    let input = b"error: bounded";
    let mut result = RichContextResult::default();
    let input_view = RichStringView {
        ptr: mojo_pointer_address(input.as_ptr()),
        len: input.len() as u64,
    };
    let status = unsafe {
        prodex_mojo_rich_context_analyze_v2(
            RICH_ABI_VERSION,
            mojo_pointer_address(&input_view),
            0,
            0,
            0,
            0,
            0,
            0,
            mojo_mut_pointer_address(&mut result),
        )
    };
    assert_eq!(status, RICH_STATUS_CAPACITY);
    assert_eq!(result.required_records, 1);
    assert!(result.required_output >= input.len() as i64);
    assert!(result.required_scratch >= 1);
}

#[test]
fn rich_catalog_handles_aliases_duplicates_and_capacity() {
    let ids = (0..1_024)
        .map(|index| format!("model-{index}"))
        .collect::<Vec<_>>();
    let aliases = vec![Vec::<&str>::new(); ids.len()];
    let models = ids
        .iter()
        .zip(&aliases)
        .map(|(id, aliases)| CatalogModel {
            id,
            aliases: aliases.as_slice(),
        })
        .collect::<Vec<_>>();
    let choices = plan_catalog_choices(&models, &[], Some("current-model")).unwrap();
    assert_eq!(choices.len(), 1_026);
    assert_eq!(choices.first(), Some(&CatalogChoice::ProviderDefault));
    assert_eq!(choices.last(), Some(&CatalogChoice::Custom));
    assert!(!choices.contains(&CatalogChoice::Current));
}

#[test]
fn rich_catalog_merge_deduplicates_aliases_against_canonical_ids() {
    let models = [CatalogModel {
        id: "gpt-5.6-sol",
        aliases: &["sol"],
    }];
    assert_eq!(
        merge_catalog_ids(&models, &["sol", "gpt-5.6-sol", "new-model"]).unwrap(),
        [2]
    );
}

#[test]
fn rich_catalog_reasoning_resolves_model_defaults_and_efforts() {
    let models = [
        CatalogReasoningModel {
            id: "gpt-5.6-luna",
            aliases: &["luna"],
            efforts: &["none", "low", "medium", "max"],
            default_effort: Some("medium"),
        },
        CatalogReasoningModel {
            id: "gpt-5.6-sol",
            aliases: &["sol"],
            efforts: &["low", "high", "ultra"],
            default_effort: Some("low"),
        },
    ];
    let plan = resolve_catalog_reasoning(&models, Some(" LUNA "), None, Some("MAX")).unwrap();
    assert_eq!(plan.model_index, Some(0));
    assert_eq!(plan.supported_efforts, ["none", "low", "medium", "max"]);
    assert_eq!(plan.default_effort.as_deref(), Some("medium"));
    assert_eq!(plan.selected_effort.as_deref(), Some("max"));
    let unsupported = resolve_catalog_reasoning(&models, Some("luna"), None, Some("ultra"));
    assert!(matches!(unsupported, Err(MojoError::Structured(issue)) if issue.kind == 5));
}

#[test]
fn rich_policy_route_plan_preserves_stable_strategy_semantics() {
    let models = [
        PolicyRouteModel {
            model: "a",
            input_cost: Some(20),
            output_cost: Some(30),
            policy_latency: Some(300),
            state_latency: Some(80),
            in_flight: 3,
            rpm_limit: Some(100),
            rpm_used: 90,
            tpm_limit: Some(10_000),
            tpm_used: 9_900,
        },
        PolicyRouteModel {
            model: "b",
            input_cost: Some(10),
            output_cost: Some(15),
            policy_latency: Some(500),
            state_latency: None,
            in_flight: 1,
            rpm_limit: Some(20),
            rpm_used: 0,
            tpm_limit: Some(100_000),
            tpm_used: 0,
        },
    ];
    assert_eq!(
        plan_route_policy("fallback", 1, 10, &models)
            .unwrap()
            .ordered_indices,
        [0, 1]
    );
    assert_eq!(
        plan_route_policy("lowest-cost", 1, 10, &models)
            .unwrap()
            .selected_index,
        Some(1)
    );
    assert_eq!(
        plan_route_policy("lowest-latency", 1, 10, &models)
            .unwrap()
            .selected_index,
        Some(0)
    );
    assert_eq!(
        plan_route_policy("round-robin", 2, 10, &models)
            .unwrap()
            .selected_index,
        Some(1)
    );
}

#[test]
fn rich_fallback_plan_deduplicates_multiple_seed_chains() {
    assert_eq!(
        model_fallback_plan("copilot", &["codex", "gpt-5.3-codex"]).unwrap(),
        ["gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"]
    );
}
