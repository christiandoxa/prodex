#[cfg(not(feature = "mojo"))]
mod rust_compat {
    use super::super::*;
    use prodex_domain::TelemetryAttributeError;

    pub fn plan_security_decision_metric(
        decision: SecurityDecisionKind,
        result: SecurityDecisionResult,
    ) -> Result<SecurityDecisionMetricPlan, TelemetryAttributeError> {
        let decision_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(132, "security_decision"),
            security_decision_kind_label(decision),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(133, "security_result"),
            security_decision_result_label(result),
        )?;
        Ok(SecurityDecisionMetricPlan {
            metric_name: crate::planning_support::metric_name(
                68,
                0,
                "prodex_security_decisions_total",
            ),
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
        let stage_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(83, "inspection_stage"),
            inspection_stage_label(stage),
        )?;
        let coverage_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(79, "inspection_coverage"),
            inspection_coverage_label(coverage),
        )?;
        let finding_category_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(80, "inspection_finding_category"),
            inspection_finding_category_label(finding_category),
        )?;
        let masking_action_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(81, "inspection_masking_action"),
            inspection_masking_action_label(masking_action),
        )?;
        let outcome_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(82, "inspection_outcome"),
            inspection_outcome_label(outcome),
        )?;
        Ok(InspectionMetricPlan {
            event_metric_name: crate::planning_support::metric_name(
                63,
                0,
                "prodex_inspection_events_total",
            ),
            duration_metric_name: crate::planning_support::metric_name(
                63,
                1,
                "prodex_inspection_duration_microseconds",
            ),
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
        let stage_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(44, "authn_validation_stage"),
            authn_token_validation_stage_label(stage),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(43, "authn_validation_result"),
            authn_token_validation_result_label(result),
        )?;
        Ok(AuthnTokenValidationMetricPlan {
            metric_name: crate::planning_support::metric_name(
                57,
                0,
                "prodex_authn_token_validation_events_total",
            ),
            increment: 1,
            stage_label,
            result_label,
        })
    }

    pub fn plan_authz_decision_metric(
        boundary: AuthzBoundaryKind,
        result: AuthzDecisionResult,
    ) -> Result<AuthzDecisionMetricPlan, TelemetryAttributeError> {
        let boundary_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(45, "authz_boundary"),
            authz_boundary_kind_label(boundary),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(46, "authz_result"),
            authz_decision_result_label(result),
        )?;
        Ok(AuthzDecisionMetricPlan {
            metric_name: crate::planning_support::metric_name(
                58,
                0,
                "prodex_authz_decisions_total",
            ),
            increment: 1,
            boundary_label,
            result_label,
        })
    }

    pub fn plan_credential_scope_mismatch_metric(
        direction: CredentialScopeMismatchDirection,
        result: CredentialScopeMismatchResult,
    ) -> Result<CredentialScopeMismatchMetricPlan, TelemetryAttributeError> {
        let direction_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(64, "credential_scope_direction"),
            credential_scope_mismatch_direction_label(direction),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(65, "credential_scope_result"),
            credential_scope_mismatch_result_label(result),
        )?;
        Ok(CredentialScopeMismatchMetricPlan {
            metric_name: crate::planning_support::metric_name(
                61,
                0,
                "prodex_credential_scope_mismatch_events_total",
            ),
            increment: 1,
            direction_label,
            result_label,
        })
    }

    pub fn plan_tenant_isolation_metric(
        surface: TenantIsolationSurface,
        result: TenantIsolationResult,
    ) -> Result<TenantIsolationMetricPlan, TelemetryAttributeError> {
        let surface_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(144, "tenant_isolation_surface"),
            tenant_isolation_surface_label(surface),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(143, "tenant_isolation_result"),
            tenant_isolation_result_label(result),
        )?;
        Ok(TenantIsolationMetricPlan {
            metric_name: crate::planning_support::metric_name(
                70,
                0,
                "prodex_tenant_isolation_events_total",
            ),
            increment: 1,
            surface_label,
            result_label,
        })
    }

    pub fn plan_postgres_tenant_context_metric(
        operation: PostgresTenantContextOperation,
        result: PostgresTenantContextResult,
    ) -> Result<PostgresTenantContextMetricPlan, TelemetryAttributeError> {
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(101, "postgres_tenant_context_operation"),
            postgres_tenant_context_operation_label(operation),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(102, "postgres_tenant_context_result"),
            postgres_tenant_context_result_label(result),
        )?;
        Ok(PostgresTenantContextMetricPlan {
            metric_name: crate::planning_support::metric_name(
                65,
                0,
                "prodex_postgres_tenant_context_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    pub fn plan_identity_context_metric(
        surface: IdentityContextSurface,
        result: IdentityContextResult,
    ) -> Result<IdentityContextMetricPlan, TelemetryAttributeError> {
        let surface_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(78, "identity_context_surface"),
            identity_context_surface_label(surface),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(77, "identity_context_result"),
            identity_context_result_label(result),
        )?;
        Ok(IdentityContextMetricPlan {
            metric_name: crate::planning_support::metric_name(
                62,
                0,
                "prodex_identity_context_events_total",
            ),
            increment: 1,
            surface_label,
            result_label,
        })
    }

    pub fn plan_break_glass_lifecycle_metric(
        operation: BreakGlassLifecycleOperation,
        result: BreakGlassLifecycleResult,
    ) -> Result<BreakGlassLifecycleMetricPlan, TelemetryAttributeError> {
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(51, "break_glass_operation"),
            break_glass_lifecycle_operation_label(operation),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(52, "break_glass_result"),
            break_glass_lifecycle_result_label(result),
        )?;
        Ok(BreakGlassLifecycleMetricPlan {
            metric_name: crate::planning_support::metric_name(
                59,
                0,
                "prodex_break_glass_lifecycle_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    pub fn plan_user_lifecycle_metric(
        operation: UserLifecycleOperation,
        result: UserLifecycleResult,
    ) -> Result<UserLifecycleMetricPlan, TelemetryAttributeError> {
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(147, "user_lifecycle_operation"),
            user_lifecycle_operation_label(operation),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(148, "user_lifecycle_result"),
            user_lifecycle_result_label(result),
        )?;
        Ok(UserLifecycleMetricPlan {
            metric_name: crate::planning_support::metric_name(
                72,
                0,
                "prodex_user_lifecycle_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    pub fn plan_service_identity_lifecycle_metric(
        operation: ServiceIdentityLifecycleOperation,
        result: ServiceIdentityLifecycleResult,
    ) -> Result<ServiceIdentityLifecycleMetricPlan, TelemetryAttributeError> {
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(134, "service_identity_operation"),
            service_identity_lifecycle_operation_label(operation),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(135, "service_identity_result"),
            service_identity_lifecycle_result_label(result),
        )?;
        Ok(ServiceIdentityLifecycleMetricPlan {
            metric_name: crate::planning_support::metric_name(
                69,
                0,
                "prodex_service_identity_lifecycle_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    pub fn plan_role_binding_lifecycle_metric(
        operation: RoleBindingLifecycleOperation,
        result: RoleBindingLifecycleResult,
    ) -> Result<RoleBindingLifecycleMetricPlan, TelemetryAttributeError> {
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(123, "role_binding_operation"),
            role_binding_lifecycle_operation_label(operation),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(124, "role_binding_result"),
            role_binding_lifecycle_result_label(result),
        )?;
        Ok(RoleBindingLifecycleMetricPlan {
            metric_name: crate::planning_support::metric_name(
                67,
                0,
                "prodex_role_binding_lifecycle_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    pub fn plan_provider_credential_lifecycle_metric(
        operation: ProviderCredentialLifecycleOperation,
        result: ProviderCredentialLifecycleResult,
    ) -> Result<ProviderCredentialLifecycleMetricPlan, TelemetryAttributeError> {
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(108, "provider_credential_operation"),
            provider_credential_lifecycle_operation_label(operation),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(109, "provider_credential_result"),
            provider_credential_lifecycle_result_label(result),
        )?;
        Ok(ProviderCredentialLifecycleMetricPlan {
            metric_name: crate::planning_support::metric_name(
                66,
                0,
                "prodex_provider_credential_lifecycle_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    pub fn plan_virtual_key_lifecycle_metric(
        operation: VirtualKeyLifecycleOperation,
        result: VirtualKeyLifecycleResult,
    ) -> Result<VirtualKeyLifecycleMetricPlan, TelemetryAttributeError> {
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(62, "credential_lifecycle_operation"),
            virtual_key_lifecycle_operation_label(operation),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(63, "credential_lifecycle_result"),
            virtual_key_lifecycle_result_label(result),
        )?;
        Ok(VirtualKeyLifecycleMetricPlan {
            metric_name: crate::planning_support::metric_name(
                73,
                0,
                "prodex_virtual_key_lifecycle_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    pub fn plan_budget_policy_lifecycle_metric(
        operation: BudgetPolicyLifecycleOperation,
        result: BudgetPolicyLifecycleResult,
    ) -> Result<BudgetPolicyLifecycleMetricPlan, TelemetryAttributeError> {
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(53, "budget_policy_operation"),
            budget_policy_lifecycle_operation_label(operation),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(54, "budget_policy_result"),
            budget_policy_lifecycle_result_label(result),
        )?;
        Ok(BudgetPolicyLifecycleMetricPlan {
            metric_name: crate::planning_support::metric_name(
                60,
                0,
                "prodex_budget_policy_lifecycle_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    pub fn plan_policy_lifecycle_metric(
        operation: PolicyLifecycleOperation,
        result: PolicyLifecycleResult,
    ) -> Result<PolicyLifecycleMetricPlan, TelemetryAttributeError> {
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(95, "policy_lifecycle_operation"),
            policy_lifecycle_operation_label(operation),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(96, "policy_lifecycle_result"),
            policy_lifecycle_result_label(result),
        )?;
        Ok(PolicyLifecycleMetricPlan {
            metric_name: crate::planning_support::metric_name(
                64,
                0,
                "prodex_policy_lifecycle_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    pub fn plan_tenant_lifecycle_metric(
        operation: TenantLifecycleOperation,
        result: TenantLifecycleResult,
    ) -> Result<TenantLifecycleMetricPlan, TelemetryAttributeError> {
        let operation_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(0, "account_lifecycle_operation"),
            tenant_lifecycle_operation_label(operation),
        )?;
        let result_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(1, "account_lifecycle_result"),
            tenant_lifecycle_result_label(result),
        )?;
        Ok(TenantLifecycleMetricPlan {
            metric_name: crate::planning_support::metric_name(
                71,
                0,
                "prodex_tenant_lifecycle_events_total",
            ),
            increment: 1,
            operation_label,
            result_label,
        })
    }

    #[cfg(not(feature = "mojo"))]
    fn security_decision_kind_label(decision: SecurityDecisionKind) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn inspection_stage_label(stage: InspectionStage) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn inspection_coverage_label(coverage: InspectionCoverageClass) -> String {
        {
            (match coverage {
                InspectionCoverageClass::Full => "full",
                InspectionCoverageClass::Partial => "partial",
                InspectionCoverageClass::Unsupported => "unsupported",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn inspection_finding_category_label(category: InspectionFindingCategory) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn inspection_masking_action_label(action: InspectionMaskingAction) -> String {
        {
            (match action {
                InspectionMaskingAction::None => "none",
                InspectionMaskingAction::Masked => "masked",
                InspectionMaskingAction::Denied => "denied",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn inspection_outcome_label(outcome: InspectionOutcome) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn security_decision_result_label(result: SecurityDecisionResult) -> String {
        {
            (match result {
                SecurityDecisionResult::Allowed => "allowed",
                SecurityDecisionResult::Denied => "denied",
                SecurityDecisionResult::Error => "error",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn authn_token_validation_stage_label(stage: AuthnTokenValidationStage) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn authn_token_validation_result_label(result: AuthnTokenValidationResult) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn authz_boundary_kind_label(boundary: AuthzBoundaryKind) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn authz_decision_result_label(result: AuthzDecisionResult) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn credential_scope_mismatch_direction_label(
        direction: CredentialScopeMismatchDirection,
    ) -> String {
        {
            (match direction {
                CredentialScopeMismatchDirection::DataPlaneToControlPlane => {
                    "data_plane_to_control_plane"
                }
                CredentialScopeMismatchDirection::ControlPlaneToDataPlane => {
                    "control_plane_to_data_plane"
                }
                CredentialScopeMismatchDirection::BreakGlassToDataPlane => {
                    "break_glass_to_data_plane"
                }
                CredentialScopeMismatchDirection::BreakGlassToControlPlane => {
                    "break_glass_to_control_plane"
                }
                CredentialScopeMismatchDirection::MissingCredential => "missing_credential",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn credential_scope_mismatch_result_label(result: CredentialScopeMismatchResult) -> String {
        {
            (match result {
                CredentialScopeMismatchResult::Rejected => "rejected",
                CredentialScopeMismatchResult::Audited => "audited",
                CredentialScopeMismatchResult::Failed => "failed",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn tenant_isolation_surface_label(surface: TenantIsolationSurface) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn tenant_isolation_result_label(result: TenantIsolationResult) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn postgres_tenant_context_operation_label(
        operation: PostgresTenantContextOperation,
    ) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn postgres_tenant_context_result_label(result: PostgresTenantContextResult) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn identity_context_surface_label(surface: IdentityContextSurface) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn identity_context_result_label(result: IdentityContextResult) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn break_glass_lifecycle_operation_label(operation: BreakGlassLifecycleOperation) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn break_glass_lifecycle_result_label(result: BreakGlassLifecycleResult) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn user_lifecycle_operation_label(operation: UserLifecycleOperation) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn user_lifecycle_result_label(result: UserLifecycleResult) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn service_identity_lifecycle_operation_label(
        operation: ServiceIdentityLifecycleOperation,
    ) -> String {
        {
            (match operation {
                ServiceIdentityLifecycleOperation::Create => "create",
                ServiceIdentityLifecycleOperation::RotateSecret => "rotate_secret",
                ServiceIdentityLifecycleOperation::Disable => "disable",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn service_identity_lifecycle_result_label(result: ServiceIdentityLifecycleResult) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn role_binding_lifecycle_operation_label(operation: RoleBindingLifecycleOperation) -> String {
        {
            (match operation {
                RoleBindingLifecycleOperation::Grant => "grant",
                RoleBindingLifecycleOperation::Revoke => "revoke",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn role_binding_lifecycle_result_label(result: RoleBindingLifecycleResult) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn provider_credential_lifecycle_operation_label(
        operation: ProviderCredentialLifecycleOperation,
    ) -> String {
        {
            (match operation {
                ProviderCredentialLifecycleOperation::Rotate => "rotate",
                ProviderCredentialLifecycleOperation::ValidateReference => "validate_reference",
                ProviderCredentialLifecycleOperation::PersistReference => "persist_reference",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn provider_credential_lifecycle_result_label(
        result: ProviderCredentialLifecycleResult,
    ) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn virtual_key_lifecycle_operation_label(operation: VirtualKeyLifecycleOperation) -> String {
        {
            (match operation {
                VirtualKeyLifecycleOperation::Create => "create",
                VirtualKeyLifecycleOperation::RotateSecret => "rotate_secret",
                VirtualKeyLifecycleOperation::PersistReference => "persist_reference",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn virtual_key_lifecycle_result_label(result: VirtualKeyLifecycleResult) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn budget_policy_lifecycle_operation_label(
        operation: BudgetPolicyLifecycleOperation,
    ) -> String {
        {
            (match operation {
                BudgetPolicyLifecycleOperation::Update => "update",
                BudgetPolicyLifecycleOperation::ValidateScope => "validate_scope",
                BudgetPolicyLifecycleOperation::PersistPolicy => "persist_policy",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn budget_policy_lifecycle_result_label(result: BudgetPolicyLifecycleResult) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn policy_lifecycle_operation_label(operation: PolicyLifecycleOperation) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn policy_lifecycle_result_label(result: PolicyLifecycleResult) -> String {
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

    #[cfg(not(feature = "mojo"))]
    fn tenant_lifecycle_operation_label(operation: TenantLifecycleOperation) -> String {
        {
            (match operation {
                TenantLifecycleOperation::Create => "create",
                TenantLifecycleOperation::Update => "update",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn tenant_lifecycle_result_label(result: TenantLifecycleResult) -> String {
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
}

#[cfg(not(feature = "mojo"))]
pub use rust_compat::*;

#[cfg(feature = "mojo")]
mod mojo_impl {
    use super::super::*;
    use prodex_domain::TelemetryAttributeError;

    macro_rules! two_label_plan {
        ($name:ident, $return_type:ident, $plan:literal, $left:ident : $left_type:ty => $left_label:ident, $right:ident : $right_type:ty => $right_label:ident) => {
            pub fn $name(
                $left: $left_type,
                $right: $right_type,
            ) -> Result<$return_type, TelemetryAttributeError> {
                Ok($return_type {
                    metric_name: crate::planning_support::metric_name($plan, 0, ""),
                    increment: 1,
                    $left_label: crate::planning_support::planned_metric_label(
                        $plan,
                        0,
                        $left as i64,
                    )?,
                    $right_label: crate::planning_support::planned_metric_label(
                        $plan,
                        1,
                        $right as i64,
                    )?,
                })
            }
        };
    }

    pub fn plan_inspection_metric(
        stage: InspectionStage,
        coverage: InspectionCoverageClass,
        finding_category: InspectionFindingCategory,
        masking_action: InspectionMaskingAction,
        outcome: InspectionOutcome,
        duration_micros: u64,
    ) -> Result<InspectionMetricPlan, TelemetryAttributeError> {
        Ok(InspectionMetricPlan {
            event_metric_name: crate::planning_support::metric_name(63, 0, ""),
            duration_metric_name: crate::planning_support::metric_name(63, 1, ""),
            increment: 1,
            duration_micros: duration_micros.min(120_000_000),
            stage_label: crate::planning_support::planned_metric_label(63, 0, stage as i64)?,
            coverage_label: crate::planning_support::planned_metric_label(63, 1, coverage as i64)?,
            finding_category_label: crate::planning_support::planned_metric_label(
                63,
                2,
                finding_category as i64,
            )?,
            masking_action_label: crate::planning_support::planned_metric_label(
                63,
                3,
                masking_action as i64,
            )?,
            outcome_label: crate::planning_support::planned_metric_label(63, 4, outcome as i64)?,
        })
    }

    two_label_plan!(plan_security_decision_metric, SecurityDecisionMetricPlan, 68, decision: SecurityDecisionKind => decision_label, result: SecurityDecisionResult => result_label);
    two_label_plan!(plan_authn_token_validation_metric, AuthnTokenValidationMetricPlan, 57, stage: AuthnTokenValidationStage => stage_label, result: AuthnTokenValidationResult => result_label);
    two_label_plan!(plan_authz_decision_metric, AuthzDecisionMetricPlan, 58, boundary: AuthzBoundaryKind => boundary_label, result: AuthzDecisionResult => result_label);
    two_label_plan!(plan_credential_scope_mismatch_metric, CredentialScopeMismatchMetricPlan, 61, direction: CredentialScopeMismatchDirection => direction_label, result: CredentialScopeMismatchResult => result_label);
    two_label_plan!(plan_tenant_isolation_metric, TenantIsolationMetricPlan, 70, surface: TenantIsolationSurface => surface_label, result: TenantIsolationResult => result_label);
    two_label_plan!(plan_postgres_tenant_context_metric, PostgresTenantContextMetricPlan, 65, operation: PostgresTenantContextOperation => operation_label, result: PostgresTenantContextResult => result_label);
    two_label_plan!(plan_identity_context_metric, IdentityContextMetricPlan, 62, surface: IdentityContextSurface => surface_label, result: IdentityContextResult => result_label);
    two_label_plan!(plan_break_glass_lifecycle_metric, BreakGlassLifecycleMetricPlan, 59, operation: BreakGlassLifecycleOperation => operation_label, result: BreakGlassLifecycleResult => result_label);
    two_label_plan!(plan_user_lifecycle_metric, UserLifecycleMetricPlan, 72, operation: UserLifecycleOperation => operation_label, result: UserLifecycleResult => result_label);
    two_label_plan!(plan_service_identity_lifecycle_metric, ServiceIdentityLifecycleMetricPlan, 69, operation: ServiceIdentityLifecycleOperation => operation_label, result: ServiceIdentityLifecycleResult => result_label);
    two_label_plan!(plan_role_binding_lifecycle_metric, RoleBindingLifecycleMetricPlan, 67, operation: RoleBindingLifecycleOperation => operation_label, result: RoleBindingLifecycleResult => result_label);
    two_label_plan!(plan_provider_credential_lifecycle_metric, ProviderCredentialLifecycleMetricPlan, 66, operation: ProviderCredentialLifecycleOperation => operation_label, result: ProviderCredentialLifecycleResult => result_label);
    two_label_plan!(plan_virtual_key_lifecycle_metric, VirtualKeyLifecycleMetricPlan, 73, operation: VirtualKeyLifecycleOperation => operation_label, result: VirtualKeyLifecycleResult => result_label);
    two_label_plan!(plan_budget_policy_lifecycle_metric, BudgetPolicyLifecycleMetricPlan, 60, operation: BudgetPolicyLifecycleOperation => operation_label, result: BudgetPolicyLifecycleResult => result_label);
    two_label_plan!(plan_policy_lifecycle_metric, PolicyLifecycleMetricPlan, 64, operation: PolicyLifecycleOperation => operation_label, result: PolicyLifecycleResult => result_label);
    two_label_plan!(plan_tenant_lifecycle_metric, TenantLifecycleMetricPlan, 71, operation: TenantLifecycleOperation => operation_label, result: TenantLifecycleResult => result_label);
}

#[cfg(feature = "mojo")]
pub use mojo_impl::*;
