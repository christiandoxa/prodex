use super::*;
use std::collections::BTreeSet;
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
fn rich_catalog_planner_sorts_filters_and_resolves_main_and_sub_agent_config() {
    let models = [
        CatalogPlanModel {
            id: "gpt-5.6-luna",
            aliases: &["luna"],
            label: "Luna",
            priority: 4,
            supported: true,
            hidden: false,
            listed: true,
            efforts: &["low", "medium", "max"],
            default_effort: Some("medium"),
        },
        CatalogPlanModel {
            id: "GPT-5.6-LUNA",
            aliases: &[],
            label: "Preferred Luna",
            priority: 1,
            supported: true,
            hidden: false,
            listed: true,
            efforts: &["medium", "max", "max"],
            default_effort: Some("medium"),
        },
        CatalogPlanModel {
            id: "hidden",
            aliases: &[],
            label: "Hidden",
            priority: 0,
            supported: true,
            hidden: true,
            listed: true,
            efforts: &["low"],
            default_effort: None,
        },
    ];
    let choices = plan_dynamic_catalog(&models).unwrap();
    assert_eq!(choices.models.len(), 1);
    assert_eq!(choices.models[0].id, "GPT-5.6-LUNA");
    assert_eq!(choices.models[0].label, "Preferred Luna");
    assert_eq!(choices.models[0].efforts, ["medium", "max"]);

    let fallback = ["none", "low", "medium", "max"];
    let main = plan_catalog_configuration(CatalogConfigurationInput {
        role: CatalogPlanRole::Main,
        models: &models[..1],
        current: None,
        provider_default: Some("gpt-5.6-luna"),
        catalog_default: Some("gpt-5.6-luna"),
        explicit_model: None,
        remembered_model: Some(" LUNA "),
        explicit_effort: None,
        remembered_effort: Some("MAX"),
        fallback_efforts: &fallback,
    })
    .unwrap();
    assert_eq!(main.selected_model.as_deref(), Some(" LUNA "));
    assert_eq!(main.selected_effort.as_deref(), Some("max"));
    assert_eq!(main.default_effort.as_deref(), Some("medium"));

    let sub = plan_catalog_configuration(CatalogConfigurationInput {
        role: CatalogPlanRole::SubAgent,
        models: &models[..1],
        current: None,
        provider_default: Some("gpt-5.6-luna"),
        catalog_default: None,
        explicit_model: Some("luna"),
        remembered_model: None,
        explicit_effort: Some("max"),
        remembered_effort: None,
        fallback_efforts: &fallback,
    })
    .unwrap();
    assert_eq!(sub.selected_model.as_deref(), Some("gpt-5.6-luna"));
    assert_eq!(sub.selected_effort.as_deref(), Some("max"));
    assert!(matches!(
        plan_catalog_configuration(CatalogConfigurationInput {
            role: CatalogPlanRole::SubAgent,
            models: &models[..1],
            current: None,
            provider_default: Some("gpt-5.6-luna"),
            catalog_default: None,
            explicit_model: Some("luna"),
            remembered_model: None,
            explicit_effort: Some("ultra"),
            remembered_effort: None,
            fallback_efforts: &fallback,
        }),
        Err(MojoError::Structured(issue)) if issue.kind == 5
    ));
}

#[test]
fn rich_catalog_planner_matches_reference_order_filter_and_effort_rules() {
    let models = [
        CatalogPlanModel {
            id: " zeta ",
            aliases: &[],
            label: " Zeta ",
            priority: 2,
            supported: true,
            hidden: false,
            listed: true,
            efforts: &[" LOW ", "low", "medium"],
            default_effort: None,
        },
        CatalogPlanModel {
            id: "alpha",
            aliases: &[],
            label: "Alpha",
            priority: 1,
            supported: true,
            hidden: false,
            listed: true,
            efforts: &["high", "low"],
            default_effort: None,
        },
        CatalogPlanModel {
            id: "ignored",
            aliases: &[],
            label: "Ignored",
            priority: 0,
            supported: false,
            hidden: false,
            listed: true,
            efforts: &[],
            default_effort: None,
        },
    ];
    let mut reference = models
        .iter()
        .filter(|model| model.supported && !model.hidden && model.listed)
        .collect::<Vec<_>>();
    reference.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| left.id.trim().cmp(right.id.trim()))
    });
    let mut seen = BTreeSet::new();
    let reference = reference
        .into_iter()
        .filter_map(|model| {
            let id = model.id.trim();
            let mut effort_seen = BTreeSet::new();
            seen.insert(id.to_ascii_lowercase())
                .then(|| CatalogPlannedModel {
                    id: id.to_string(),
                    label: model.label.trim().to_string(),
                    efforts: model
                        .efforts
                        .iter()
                        .map(|effort| effort.trim())
                        .filter(|effort| !effort.is_empty())
                        .filter(|effort| effort_seen.insert(effort.to_ascii_lowercase()))
                        .map(str::to_string)
                        .collect(),
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(plan_dynamic_catalog(&models).unwrap().models, reference);
}

fn rust_catalog_configuration_oracle(
    input: CatalogConfigurationInput<'_>,
) -> Result<CatalogConfigurationPlan, ()> {
    let find = |query: &str| {
        let query = query.trim();
        if query.is_empty() {
            return None;
        }
        input.models.iter().position(|model| {
            model.id.trim().eq_ignore_ascii_case(query)
                || model
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(query))
        })
    };
    let mut selected_model = None;
    let mut selected_index = None;
    if let Some(model) = input.explicit_model {
        selected_model = Some(match input.role {
            CatalogPlanRole::Main => model.to_string(),
            CatalogPlanRole::SubAgent => find(model)
                .map(|index| input.models[index].id.to_string())
                .unwrap_or_else(|| model.to_string()),
        });
        selected_index = find(model);
    } else if input.role == CatalogPlanRole::Main {
        for candidate in [input.remembered_model, input.current] {
            if let Some(candidate) = candidate
                && let Some(index) = find(candidate)
            {
                selected_model = Some(candidate.to_string());
                selected_index = Some(index);
                break;
            }
        }
        if selected_model.is_none()
            && let Some(candidate) = [input.catalog_default, input.provider_default]
                .into_iter()
                .flatten()
                .next()
        {
            selected_model = Some(candidate.to_string());
            selected_index = find(candidate);
        }
    }
    let effort_index = selected_index.or_else(|| {
        (input.role == CatalogPlanRole::SubAgent)
            .then(|| input.provider_default.and_then(find))
            .flatten()
    });
    let effort_values = effort_index
        .map(|index| input.models[index].efforts)
        .unwrap_or(input.fallback_efforts);
    let mut seen = BTreeSet::new();
    let supported_efforts = effort_values
        .iter()
        .map(|effort| effort.trim())
        .filter(|effort| !effort.is_empty())
        .filter(|effort| seen.insert(effort.to_ascii_lowercase()))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let default_effort = effort_index
        .and_then(|index| input.models[index].default_effort)
        .map(str::trim)
        .filter(|effort| !effort.is_empty())
        .map(str::to_string)
        .or_else(|| supported_efforts.first().cloned());
    let selected_effort = if let Some(requested) = input.explicit_effort {
        Some(
            supported_efforts
                .iter()
                .find(|effort| effort.eq_ignore_ascii_case(requested.trim()))
                .cloned()
                .ok_or(())?,
        )
    } else if input.role == CatalogPlanRole::Main
        && let (Some(selected), Some(remembered)) =
            (selected_model.as_deref(), input.remembered_model)
        && selected.trim().eq_ignore_ascii_case(remembered.trim())
    {
        supported_efforts
            .iter()
            .find(|effort| {
                input
                    .remembered_effort
                    .is_some_and(|remembered| effort.eq_ignore_ascii_case(remembered.trim()))
            })
            .cloned()
            .or_else(|| default_effort.clone())
    } else {
        default_effort.clone()
    };
    Ok(CatalogConfigurationPlan {
        selected_model,
        selected_effort,
        default_effort,
    })
}

#[test]
fn rich_catalog_configuration_matches_test_only_rust_oracle() {
    let models = [
        CatalogPlanModel {
            id: "gpt-5.6-sol",
            aliases: &["sol"],
            label: "Sol",
            priority: 1,
            supported: true,
            hidden: false,
            listed: true,
            efforts: &["low", " MEDIUM ", "medium"],
            default_effort: Some("low"),
        },
        CatalogPlanModel {
            id: "gpt-5.6-luna",
            aliases: &["luna"],
            label: "Luna",
            priority: 2,
            supported: true,
            hidden: false,
            listed: true,
            efforts: &["none", "max"],
            default_effort: Some("max"),
        },
    ];
    let fallback = ["none", "low", "medium", "high"];
    let cases = [
        CatalogConfigurationInput {
            role: CatalogPlanRole::Main,
            models: &models,
            current: Some("current-model"),
            provider_default: Some("gpt-5.6-sol"),
            catalog_default: Some("gpt-5.6-luna"),
            explicit_model: None,
            remembered_model: Some(" SOL "),
            explicit_effort: None,
            remembered_effort: Some("MEDIUM"),
            fallback_efforts: &fallback,
        },
        CatalogConfigurationInput {
            role: CatalogPlanRole::Main,
            models: &models,
            current: None,
            provider_default: Some("gpt-5.6-luna"),
            catalog_default: Some("gpt-5.6-sol"),
            explicit_model: Some("dynamic/model"),
            remembered_model: Some("gpt-5.6-sol"),
            explicit_effort: Some(" HIGH "),
            remembered_effort: Some("low"),
            fallback_efforts: &fallback,
        },
        CatalogConfigurationInput {
            role: CatalogPlanRole::SubAgent,
            models: &models,
            current: None,
            provider_default: Some("luna"),
            catalog_default: None,
            explicit_model: Some("sol"),
            remembered_model: None,
            explicit_effort: Some("medium"),
            remembered_effort: None,
            fallback_efforts: &fallback,
        },
        CatalogConfigurationInput {
            role: CatalogPlanRole::SubAgent,
            models: &models,
            current: None,
            provider_default: Some("luna"),
            catalog_default: None,
            explicit_model: None,
            remembered_model: None,
            explicit_effort: None,
            remembered_effort: None,
            fallback_efforts: &fallback,
        },
        CatalogConfigurationInput {
            role: CatalogPlanRole::Main,
            models: &models,
            current: None,
            provider_default: Some("gpt-5.6-sol"),
            catalog_default: None,
            explicit_model: Some("sol"),
            remembered_model: None,
            explicit_effort: Some("max"),
            remembered_effort: None,
            fallback_efforts: &fallback,
        },
    ];
    for input in cases {
        let expected = rust_catalog_configuration_oracle(input);
        let actual = plan_catalog_configuration(input);
        assert_eq!(actual.is_err(), expected.is_err());
        if let (Ok(actual), Ok(expected)) = (actual, expected) {
            assert_eq!(actual, expected);
        }
    }

    let default_only = [CatalogPlanModel {
        id: "model",
        aliases: &[],
        label: "Model",
        priority: 0,
        supported: true,
        hidden: false,
        listed: true,
        efforts: &[],
        default_effort: Some("default-effort"),
    }];
    let plan = plan_catalog_configuration(CatalogConfigurationInput {
        role: CatalogPlanRole::Main,
        models: &default_only,
        current: None,
        provider_default: Some("model"),
        catalog_default: None,
        explicit_model: None,
        remembered_model: None,
        explicit_effort: None,
        remembered_effort: None,
        fallback_efforts: &[],
    })
    .unwrap();
    assert_eq!(plan.default_effort.as_deref(), Some("default-effort"));
    assert_eq!(plan.selected_effort.as_deref(), Some("default-effort"));
}

#[test]
fn rich_catalog_planner_rejects_oversized_inputs_without_panicking() {
    let ids = (0..1_025)
        .map(|index| format!("model-{index}"))
        .collect::<Vec<_>>();
    let models = ids
        .iter()
        .enumerate()
        .map(|(index, id)| CatalogPlanModel {
            id,
            aliases: &[],
            label: "Model",
            priority: index as u64,
            supported: true,
            hidden: false,
            listed: true,
            efforts: &[],
            default_effort: None,
        })
        .collect::<Vec<_>>();
    assert_eq!(plan_dynamic_catalog(&models), Err(MojoError::InvalidInput));

    let oversized_id = "x".repeat(4_097);
    let model = [CatalogPlanModel {
        id: &oversized_id,
        aliases: &[],
        label: "Model",
        priority: 0,
        supported: true,
        hidden: false,
        listed: true,
        efforts: &[],
        default_effort: None,
    }];
    assert_eq!(plan_dynamic_catalog(&model), Err(MojoError::InvalidInput));

    let model = [CatalogPlanModel {
        id: "model",
        aliases: &[],
        label: "Model",
        priority: u64::MAX,
        supported: true,
        hidden: false,
        listed: true,
        efforts: &[],
        default_effort: None,
    }];
    assert_eq!(plan_dynamic_catalog(&model), Err(MojoError::InvalidInput));
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
