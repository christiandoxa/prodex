#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::kiro_environment::{
    KiroMcpDisabledToolsState, KiroMcpNormalizationAction, KiroMcpServerFacts,
    plan_mcp_normalization,
};

#[test]
fn kiro_environment_required_mojo_matrix_plans_targeted_and_untargeted_servers() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    let facts = [
        KiroMcpServerFacts {
            server_name_matches: 1,
            disabled_tools_state: KiroMcpDisabledToolsState::Missing,
            ..KiroMcpServerFacts::default()
        },
        KiroMcpServerFacts {
            command_name_matches: 1,
            disabled_tools_state: KiroMcpDisabledToolsState::Array,
            ..KiroMcpServerFacts::default()
        },
        KiroMcpServerFacts {
            command_name_matches: 1,
            disabled_tools_state: KiroMcpDisabledToolsState::Array,
            incompatible_tool_present: 1,
            ..KiroMcpServerFacts::default()
        },
        KiroMcpServerFacts {
            disabled_tools_state: KiroMcpDisabledToolsState::Invalid,
            ..KiroMcpServerFacts::default()
        },
    ];

    assert_eq!(
        plan_mcp_normalization(&facts).unwrap(),
        [
            KiroMcpNormalizationAction::InitializeDisabledTools,
            KiroMcpNormalizationAction::AppendIncompatibleTool,
            KiroMcpNormalizationAction::Keep,
            KiroMcpNormalizationAction::Keep,
        ]
    );
}

#[test]
fn kiro_environment_required_mojo_abi_rejects_invalid_fact_and_capacity() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    assert_eq!(
        plan_mcp_normalization(&[KiroMcpServerFacts {
            server_name_matches: 2,
            ..KiroMcpServerFacts::default()
        }]),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );
    assert_eq!(
        plan_mcp_normalization(&vec![KiroMcpServerFacts::default(); 65_537]),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );
}
