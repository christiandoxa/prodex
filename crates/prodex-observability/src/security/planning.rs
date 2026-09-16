use super::*;
use prodex_domain::{TelemetryAttribute, TelemetryAttributeError};

pub fn plan_security_decision_metric(
    decision: SecurityDecisionKind,
    result: SecurityDecisionResult,
) -> Result<SecurityDecisionMetricPlan, TelemetryAttributeError> {
    let decision_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(132)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "security_decision"
            }
        },
        security_decision_kind_label(decision),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(133)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "security_result"
            }
        },
        security_decision_result_label(result),
    );
    decision_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(SecurityDecisionMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(68, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_security_decisions_total"
            }
        },
        increment: 1,
        decision_label,
        result_label,
    })
}

pub fn plan_inspection_metric(
    stage: InspectionStage,
    coverage: InspectionCoverageClass,
    finding_category: InspectionFindingCategory,
    masking_action: InspectionMaskingAction,
    outcome: InspectionOutcome,
    duration_micros: u64,
) -> Result<InspectionMetricPlan, TelemetryAttributeError> {
    let stage_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(83)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "inspection_stage"
            }
        },
        inspection_stage_label(stage),
    );
    let coverage_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(79)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "inspection_coverage"
            }
        },
        inspection_coverage_label(coverage),
    );
    let finding_category_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(80)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "inspection_finding_category"
            }
        },
        inspection_finding_category_label(finding_category),
    );
    let masking_action_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(81)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "inspection_masking_action"
            }
        },
        inspection_masking_action_label(masking_action),
    );
    let outcome_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(82)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "inspection_outcome"
            }
        },
        inspection_outcome_label(outcome),
    );
    for label in [
        &stage_label,
        &coverage_label,
        &finding_category_label,
        &masking_action_label,
        &outcome_label,
    ] {
        label.as_metric_label()?;
    }
    Ok(InspectionMetricPlan {
        event_metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(63, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_inspection_events_total"
            }
        },
        duration_metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(63, 1)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_inspection_duration_microseconds"
            }
        },
        increment: 1,
        duration_micros: duration_micros.min(120_000_000),
        stage_label,
        coverage_label,
        finding_category_label,
        masking_action_label,
        outcome_label,
    })
}

pub fn plan_authn_token_validation_metric(
    stage: AuthnTokenValidationStage,
    result: AuthnTokenValidationResult,
) -> Result<AuthnTokenValidationMetricPlan, TelemetryAttributeError> {
    let stage_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(44)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "authn_validation_stage"
            }
        },
        authn_token_validation_stage_label(stage),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(43)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "authn_validation_result"
            }
        },
        authn_token_validation_result_label(result),
    );
    stage_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(AuthnTokenValidationMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(57, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_authn_token_validation_events_total"
            }
        },
        increment: 1,
        stage_label,
        result_label,
    })
}

pub fn plan_authz_decision_metric(
    boundary: AuthzBoundaryKind,
    result: AuthzDecisionResult,
) -> Result<AuthzDecisionMetricPlan, TelemetryAttributeError> {
    let boundary_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(45)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "authz_boundary"
            }
        },
        authz_boundary_kind_label(boundary),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(46)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "authz_result"
            }
        },
        authz_decision_result_label(result),
    );
    boundary_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(AuthzDecisionMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(58, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_authz_decisions_total"
            }
        },
        increment: 1,
        boundary_label,
        result_label,
    })
}

pub fn plan_credential_scope_mismatch_metric(
    direction: CredentialScopeMismatchDirection,
    result: CredentialScopeMismatchResult,
) -> Result<CredentialScopeMismatchMetricPlan, TelemetryAttributeError> {
    let direction_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(64)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "credential_scope_direction"
            }
        },
        credential_scope_mismatch_direction_label(direction),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(65)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "credential_scope_result"
            }
        },
        credential_scope_mismatch_result_label(result),
    );
    direction_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(CredentialScopeMismatchMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(61, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_credential_scope_mismatch_events_total"
            }
        },
        increment: 1,
        direction_label,
        result_label,
    })
}

pub fn plan_tenant_isolation_metric(
    surface: TenantIsolationSurface,
    result: TenantIsolationResult,
) -> Result<TenantIsolationMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(144)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "tenant_isolation_surface"
            }
        },
        tenant_isolation_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(143)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "tenant_isolation_result"
            }
        },
        tenant_isolation_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(TenantIsolationMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(70, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_tenant_isolation_events_total"
            }
        },
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_postgres_tenant_context_metric(
    operation: PostgresTenantContextOperation,
    result: PostgresTenantContextResult,
) -> Result<PostgresTenantContextMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(101)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "postgres_tenant_context_operation"
            }
        },
        postgres_tenant_context_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(102)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "postgres_tenant_context_result"
            }
        },
        postgres_tenant_context_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(PostgresTenantContextMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(65, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_postgres_tenant_context_events_total"
            }
        },
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_identity_context_metric(
    surface: IdentityContextSurface,
    result: IdentityContextResult,
) -> Result<IdentityContextMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(78)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "identity_context_surface"
            }
        },
        identity_context_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(77)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "identity_context_result"
            }
        },
        identity_context_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(IdentityContextMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(62, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_identity_context_events_total"
            }
        },
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_break_glass_lifecycle_metric(
    operation: BreakGlassLifecycleOperation,
    result: BreakGlassLifecycleResult,
) -> Result<BreakGlassLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(51)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "break_glass_operation"
            }
        },
        break_glass_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(52)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "break_glass_result"
            }
        },
        break_glass_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(BreakGlassLifecycleMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(59, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_break_glass_lifecycle_events_total"
            }
        },
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_user_lifecycle_metric(
    operation: UserLifecycleOperation,
    result: UserLifecycleResult,
) -> Result<UserLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(147)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "user_lifecycle_operation"
            }
        },
        user_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(148)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "user_lifecycle_result"
            }
        },
        user_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(UserLifecycleMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(72, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_user_lifecycle_events_total"
            }
        },
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_service_identity_lifecycle_metric(
    operation: ServiceIdentityLifecycleOperation,
    result: ServiceIdentityLifecycleResult,
) -> Result<ServiceIdentityLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(134)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "service_identity_operation"
            }
        },
        service_identity_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(135)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "service_identity_result"
            }
        },
        service_identity_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ServiceIdentityLifecycleMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(69, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_service_identity_lifecycle_events_total"
            }
        },
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_role_binding_lifecycle_metric(
    operation: RoleBindingLifecycleOperation,
    result: RoleBindingLifecycleResult,
) -> Result<RoleBindingLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(123)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "role_binding_operation"
            }
        },
        role_binding_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(124)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "role_binding_result"
            }
        },
        role_binding_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(RoleBindingLifecycleMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(67, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_role_binding_lifecycle_events_total"
            }
        },
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_provider_credential_lifecycle_metric(
    operation: ProviderCredentialLifecycleOperation,
    result: ProviderCredentialLifecycleResult,
) -> Result<ProviderCredentialLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(108)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "provider_credential_operation"
            }
        },
        provider_credential_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(109)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "provider_credential_result"
            }
        },
        provider_credential_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ProviderCredentialLifecycleMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(66, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_provider_credential_lifecycle_events_total"
            }
        },
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_virtual_key_lifecycle_metric(
    operation: VirtualKeyLifecycleOperation,
    result: VirtualKeyLifecycleResult,
) -> Result<VirtualKeyLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(62)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "credential_lifecycle_operation"
            }
        },
        virtual_key_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(63)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "credential_lifecycle_result"
            }
        },
        virtual_key_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(VirtualKeyLifecycleMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(73, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_virtual_key_lifecycle_events_total"
            }
        },
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_budget_policy_lifecycle_metric(
    operation: BudgetPolicyLifecycleOperation,
    result: BudgetPolicyLifecycleResult,
) -> Result<BudgetPolicyLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(53)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "budget_policy_operation"
            }
        },
        budget_policy_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(54)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "budget_policy_result"
            }
        },
        budget_policy_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(BudgetPolicyLifecycleMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(60, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_budget_policy_lifecycle_events_total"
            }
        },
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_policy_lifecycle_metric(
    operation: PolicyLifecycleOperation,
    result: PolicyLifecycleResult,
) -> Result<PolicyLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(95)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "policy_lifecycle_operation"
            }
        },
        policy_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(96)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "policy_lifecycle_result"
            }
        },
        policy_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(PolicyLifecycleMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(64, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_policy_lifecycle_events_total"
            }
        },
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_tenant_lifecycle_metric(
    operation: TenantLifecycleOperation,
    result: TenantLifecycleResult,
) -> Result<TenantLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "account_lifecycle_operation"
            }
        },
        tenant_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::label_key(1)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "account_lifecycle_result"
            }
        },
        tenant_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(TenantLifecycleMetricPlan {
        metric_name: {
            #[cfg(feature = "mojo")]
            {
                crate::mojo::metric_name(71, 0)
            }
            #[cfg(not(feature = "mojo"))]
            {
                "prodex_tenant_lifecycle_events_total"
            }
        },
        increment: 1,
        operation_label,
        result_label,
    })
}

fn security_decision_kind_label(decision: SecurityDecisionKind) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(124, decision as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match decision {
            SecurityDecisionKind::Authentication => "authentication",
            SecurityDecisionKind::TenantResolution => "tenant_resolution",
            SecurityDecisionKind::Authorization => "authorization",
            SecurityDecisionKind::CredentialScope => "credential_scope",
        })
        .to_string()
    }
}

fn inspection_stage_label(stage: InspectionStage) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(119, stage as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match stage {
            InspectionStage::Local => "local",
            InspectionStage::External => "external",
            InspectionStage::Merge => "merge",
            InspectionStage::RequestEnforcement => "request_enforcement",
            InspectionStage::ResponseEnforcement => "response_enforcement",
        })
        .to_string()
    }
}

fn inspection_coverage_label(coverage: InspectionCoverageClass) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(115, coverage as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match coverage {
            InspectionCoverageClass::Full => "full",
            InspectionCoverageClass::Partial => "partial",
            InspectionCoverageClass::Unsupported => "unsupported",
        })
        .to_string()
    }
}

fn inspection_finding_category_label(category: InspectionFindingCategory) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(116, category as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match category {
            InspectionFindingCategory::None => "none",
            InspectionFindingCategory::PersonalData => "personal_data",
            InspectionFindingCategory::Credential => "credential",
            InspectionFindingCategory::Financial => "financial",
            InspectionFindingCategory::Multiple => "multiple",
        })
        .to_string()
    }
}

fn inspection_masking_action_label(action: InspectionMaskingAction) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(117, action as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match action {
            InspectionMaskingAction::None => "none",
            InspectionMaskingAction::Masked => "masked",
            InspectionMaskingAction::Denied => "denied",
        })
        .to_string()
    }
}

fn inspection_outcome_label(outcome: InspectionOutcome) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(118, outcome as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match outcome {
            InspectionOutcome::Allowed => "allowed",
            InspectionOutcome::Denied => "denied",
            InspectionOutcome::Timeout => "timeout",
            InspectionOutcome::Error => "error",
        })
        .to_string()
    }
}

fn security_decision_result_label(result: SecurityDecisionResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(125, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            SecurityDecisionResult::Allowed => "allowed",
            SecurityDecisionResult::Denied => "denied",
            SecurityDecisionResult::Error => "error",
        })
        .to_string()
    }
}

fn authn_token_validation_stage_label(stage: AuthnTokenValidationStage) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(106, stage as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match stage {
            AuthnTokenValidationStage::Decode => "decode",
            AuthnTokenValidationStage::Signature => "signature",
            AuthnTokenValidationStage::Claims => "claims",
            AuthnTokenValidationStage::TenantClaim => "tenant_claim",
            AuthnTokenValidationStage::RoleClaim => "role_claim",
            AuthnTokenValidationStage::JwksCache => "jwks_cache",
        })
        .to_string()
    }
}

fn authn_token_validation_result_label(result: AuthnTokenValidationResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(105, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            AuthnTokenValidationResult::Accepted => "accepted",
            AuthnTokenValidationResult::Malformed => "malformed",
            AuthnTokenValidationResult::InvalidSignature => "invalid_signature",
            AuthnTokenValidationResult::Expired => "expired",
            AuthnTokenValidationResult::UnknownKey => "unknown_key",
            AuthnTokenValidationResult::MissingTenant => "missing_tenant",
            AuthnTokenValidationResult::RoleDenied => "role_denied",
            AuthnTokenValidationResult::CacheUnavailable => "cache_unavailable",
        })
        .to_string()
    }
}

fn authz_boundary_kind_label(boundary: AuthzBoundaryKind) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(107, boundary as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match boundary {
            AuthzBoundaryKind::DataPlaneInference => "data_plane_inference",
            AuthzBoundaryKind::DataPlaneQuota => "data_plane_quota",
            AuthzBoundaryKind::ControlPlaneRead => "control_plane_read",
            AuthzBoundaryKind::ControlPlaneMutation => "control_plane_mutation",
            AuthzBoundaryKind::ControlPlaneBilling => "control_plane_billing",
            AuthzBoundaryKind::BreakGlass => "break_glass",
        })
        .to_string()
    }
}

fn authz_decision_result_label(result: AuthzDecisionResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(108, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            AuthzDecisionResult::Allowed => "allowed",
            AuthzDecisionResult::CredentialScopeDenied => "credential_scope_denied",
            AuthzDecisionResult::RoleDenied => "role_denied",
            AuthzDecisionResult::TenantDenied => "tenant_denied",
            AuthzDecisionResult::ResourceDenied => "resource_denied",
            AuthzDecisionResult::Failed => "failed",
        })
        .to_string()
    }
}

fn credential_scope_mismatch_direction_label(
    direction: CredentialScopeMismatchDirection,
) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(140, direction as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match direction {
            CredentialScopeMismatchDirection::DataPlaneToControlPlane => {
                "data_plane_to_control_plane"
            }
            CredentialScopeMismatchDirection::ControlPlaneToDataPlane => {
                "control_plane_to_data_plane"
            }
            CredentialScopeMismatchDirection::BreakGlassToDataPlane => "break_glass_to_data_plane",
            CredentialScopeMismatchDirection::BreakGlassToControlPlane => {
                "break_glass_to_control_plane"
            }
            CredentialScopeMismatchDirection::MissingCredential => "missing_credential",
        })
        .to_string()
    }
}

fn credential_scope_mismatch_result_label(result: CredentialScopeMismatchResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(112, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            CredentialScopeMismatchResult::Rejected => "rejected",
            CredentialScopeMismatchResult::Audited => "audited",
            CredentialScopeMismatchResult::Failed => "failed",
        })
        .to_string()
    }
}

fn tenant_isolation_surface_label(surface: TenantIsolationSurface) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(128, surface as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match surface {
            TenantIsolationSurface::Authentication => "authentication",
            TenantIsolationSurface::Authorization => "authorization",
            TenantIsolationSurface::StoragePredicate => "storage_predicate",
            TenantIsolationSurface::CacheKey => "cache_key",
            TenantIsolationSurface::AuditQuery => "audit_query",
        })
        .to_string()
    }
}

fn tenant_isolation_result_label(result: TenantIsolationResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(127, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            TenantIsolationResult::Enforced => "enforced",
            TenantIsolationResult::CrossTenantDenied => "cross_tenant_denied",
            TenantIsolationResult::MissingTenantDenied => "missing_tenant_denied",
            TenantIsolationResult::MismatchRejected => "mismatch_rejected",
            TenantIsolationResult::Failed => "failed",
        })
        .to_string()
    }
}

fn postgres_tenant_context_operation_label(operation: PostgresTenantContextOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(141, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            PostgresTenantContextOperation::SetContext => "set_context",
            PostgresTenantContextOperation::VerifyContext => "verify_context",
            PostgresTenantContextOperation::ApplyRlsPolicy => "apply_rls_policy",
            PostgresTenantContextOperation::ExecuteTenantDml => "execute_tenant_dml",
        })
        .to_string()
    }
}

fn postgres_tenant_context_result_label(result: PostgresTenantContextResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(122, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            PostgresTenantContextResult::Applied => "applied",
            PostgresTenantContextResult::Missing => "missing",
            PostgresTenantContextResult::MismatchRejected => "mismatch_rejected",
            PostgresTenantContextResult::RlsDenied => "rls_denied",
            PostgresTenantContextResult::Failed => "failed",
        })
        .to_string()
    }
}

fn identity_context_surface_label(surface: IdentityContextSurface) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(114, surface as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match surface {
            IdentityContextSurface::Authentication => "authentication",
            IdentityContextSurface::Authorization => "authorization",
            IdentityContextSurface::Audit => "audit",
            IdentityContextSurface::ControlPlane => "control_plane",
            IdentityContextSurface::DataPlane => "data_plane",
        })
        .to_string()
    }
}

fn identity_context_result_label(result: IdentityContextResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(113, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            IdentityContextResult::Consistent => "consistent",
            IdentityContextResult::MissingPrincipal => "missing_principal",
            IdentityContextResult::MissingTenant => "missing_tenant",
            IdentityContextResult::TenantMismatch => "tenant_mismatch",
            IdentityContextResult::CorrelationMissing => "correlation_missing",
            IdentityContextResult::Failed => "failed",
        })
        .to_string()
    }
}

fn break_glass_lifecycle_operation_label(operation: BreakGlassLifecycleOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(109, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            BreakGlassLifecycleOperation::Request => "request",
            BreakGlassLifecycleOperation::Approve => "approve",
            BreakGlassLifecycleOperation::Activate => "activate",
            BreakGlassLifecycleOperation::Revoke => "revoke",
            BreakGlassLifecycleOperation::Expire => "expire",
        })
        .to_string()
    }
}

fn break_glass_lifecycle_result_label(result: BreakGlassLifecycleResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(110, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            BreakGlassLifecycleResult::Authorized => "authorized",
            BreakGlassLifecycleResult::Denied => "denied",
            BreakGlassLifecycleResult::Persisted => "persisted",
            BreakGlassLifecycleResult::Expired => "expired",
            BreakGlassLifecycleResult::Failed => "failed",
        })
        .to_string()
    }
}

fn user_lifecycle_operation_label(operation: UserLifecycleOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(131, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            UserLifecycleOperation::Invite => "invite",
            UserLifecycleOperation::ScimCreate => "scim_create",
            UserLifecycleOperation::ScimUpdate => "scim_update",
            UserLifecycleOperation::ScimDelete => "scim_delete",
        })
        .to_string()
    }
}

fn user_lifecycle_result_label(result: UserLifecycleResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(132, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            UserLifecycleResult::Authorized => "authorized",
            UserLifecycleResult::Denied => "denied",
            UserLifecycleResult::Persisted => "persisted",
            UserLifecycleResult::Failed => "failed",
        })
        .to_string()
    }
}

fn service_identity_lifecycle_operation_label(
    operation: ServiceIdentityLifecycleOperation,
) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(142, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            ServiceIdentityLifecycleOperation::Create => "create",
            ServiceIdentityLifecycleOperation::RotateSecret => "rotate_secret",
            ServiceIdentityLifecycleOperation::Disable => "disable",
        })
        .to_string()
    }
}

fn service_identity_lifecycle_result_label(result: ServiceIdentityLifecycleResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(126, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ServiceIdentityLifecycleResult::Authorized => "authorized",
            ServiceIdentityLifecycleResult::Denied => "denied",
            ServiceIdentityLifecycleResult::Persisted => "persisted",
            ServiceIdentityLifecycleResult::Failed => "failed",
        })
        .to_string()
    }
}

fn role_binding_lifecycle_operation_label(operation: RoleBindingLifecycleOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(143, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            RoleBindingLifecycleOperation::Grant => "grant",
            RoleBindingLifecycleOperation::Revoke => "revoke",
        })
        .to_string()
    }
}

fn role_binding_lifecycle_result_label(result: RoleBindingLifecycleResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(123, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            RoleBindingLifecycleResult::Authorized => "authorized",
            RoleBindingLifecycleResult::Denied => "denied",
            RoleBindingLifecycleResult::Persisted => "persisted",
            RoleBindingLifecycleResult::Failed => "failed",
        })
        .to_string()
    }
}

fn provider_credential_lifecycle_operation_label(
    operation: ProviderCredentialLifecycleOperation,
) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(144, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            ProviderCredentialLifecycleOperation::Rotate => "rotate",
            ProviderCredentialLifecycleOperation::ValidateReference => "validate_reference",
            ProviderCredentialLifecycleOperation::PersistReference => "persist_reference",
        })
        .to_string()
    }
}

fn provider_credential_lifecycle_result_label(result: ProviderCredentialLifecycleResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(145, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ProviderCredentialLifecycleResult::Authorized => "authorized",
            ProviderCredentialLifecycleResult::Denied => "denied",
            ProviderCredentialLifecycleResult::Persisted => "persisted",
            ProviderCredentialLifecycleResult::Failed => "failed",
        })
        .to_string()
    }
}

fn virtual_key_lifecycle_operation_label(operation: VirtualKeyLifecycleOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(133, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            VirtualKeyLifecycleOperation::Create => "create",
            VirtualKeyLifecycleOperation::RotateSecret => "rotate_secret",
            VirtualKeyLifecycleOperation::PersistReference => "persist_reference",
        })
        .to_string()
    }
}

fn virtual_key_lifecycle_result_label(result: VirtualKeyLifecycleResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(134, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            VirtualKeyLifecycleResult::Authorized => "authorized",
            VirtualKeyLifecycleResult::Denied => "denied",
            VirtualKeyLifecycleResult::Persisted => "persisted",
            VirtualKeyLifecycleResult::Failed => "failed",
        })
        .to_string()
    }
}

fn budget_policy_lifecycle_operation_label(operation: BudgetPolicyLifecycleOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(146, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            BudgetPolicyLifecycleOperation::Update => "update",
            BudgetPolicyLifecycleOperation::ValidateScope => "validate_scope",
            BudgetPolicyLifecycleOperation::PersistPolicy => "persist_policy",
        })
        .to_string()
    }
}

fn budget_policy_lifecycle_result_label(result: BudgetPolicyLifecycleResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(111, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            BudgetPolicyLifecycleResult::Authorized => "authorized",
            BudgetPolicyLifecycleResult::Denied => "denied",
            BudgetPolicyLifecycleResult::Persisted => "persisted",
            BudgetPolicyLifecycleResult::Failed => "failed",
        })
        .to_string()
    }
}

fn policy_lifecycle_operation_label(operation: PolicyLifecycleOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(120, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            PolicyLifecycleOperation::Create => "create",
            PolicyLifecycleOperation::Update => "update",
            PolicyLifecycleOperation::Publish => "publish",
            PolicyLifecycleOperation::Invalidate => "invalidate",
        })
        .to_string()
    }
}

fn policy_lifecycle_result_label(result: PolicyLifecycleResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(121, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            PolicyLifecycleResult::Authorized => "authorized",
            PolicyLifecycleResult::Denied => "denied",
            PolicyLifecycleResult::Persisted => "persisted",
            PolicyLifecycleResult::Published => "published",
            PolicyLifecycleResult::Failed => "failed",
        })
        .to_string()
    }
}

fn tenant_lifecycle_operation_label(operation: TenantLifecycleOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(129, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            TenantLifecycleOperation::Create => "create",
            TenantLifecycleOperation::Update => "update",
        })
        .to_string()
    }
}

fn tenant_lifecycle_result_label(result: TenantLifecycleResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(130, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            TenantLifecycleResult::Authorized => "authorized",
            TenantLifecycleResult::Denied => "denied",
            TenantLifecycleResult::Persisted => "persisted",
            TenantLifecycleResult::Failed => "failed",
        })
        .to_string()
    }
}
