use super::*;
use prodex_domain::{TelemetryAttribute, TelemetryAttributeError};

pub fn plan_security_decision_metric(
    decision: SecurityDecisionKind,
    result: SecurityDecisionResult,
) -> Result<SecurityDecisionMetricPlan, TelemetryAttributeError> {
    let decision_label = TelemetryAttribute::metric_label(
        "security_decision",
        security_decision_kind_label(decision),
    );
    let result_label =
        TelemetryAttribute::metric_label("security_result", security_decision_result_label(result));
    decision_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(SecurityDecisionMetricPlan {
        metric_name: "prodex_security_decisions_total",
        increment: 1,
        decision_label,
        result_label,
    })
}

pub fn plan_authn_token_validation_metric(
    stage: AuthnTokenValidationStage,
    result: AuthnTokenValidationResult,
) -> Result<AuthnTokenValidationMetricPlan, TelemetryAttributeError> {
    let stage_label = TelemetryAttribute::metric_label(
        "authn_validation_stage",
        authn_token_validation_stage_label(stage),
    );
    let result_label = TelemetryAttribute::metric_label(
        "authn_validation_result",
        authn_token_validation_result_label(result),
    );
    stage_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(AuthnTokenValidationMetricPlan {
        metric_name: "prodex_authn_token_validation_events_total",
        increment: 1,
        stage_label,
        result_label,
    })
}

pub fn plan_authz_decision_metric(
    boundary: AuthzBoundaryKind,
    result: AuthzDecisionResult,
) -> Result<AuthzDecisionMetricPlan, TelemetryAttributeError> {
    let boundary_label =
        TelemetryAttribute::metric_label("authz_boundary", authz_boundary_kind_label(boundary));
    let result_label =
        TelemetryAttribute::metric_label("authz_result", authz_decision_result_label(result));
    boundary_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(AuthzDecisionMetricPlan {
        metric_name: "prodex_authz_decisions_total",
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
        "credential_scope_direction",
        credential_scope_mismatch_direction_label(direction),
    );
    let result_label = TelemetryAttribute::metric_label(
        "credential_scope_result",
        credential_scope_mismatch_result_label(result),
    );
    direction_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(CredentialScopeMismatchMetricPlan {
        metric_name: "prodex_credential_scope_mismatch_events_total",
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
        "tenant_isolation_surface",
        tenant_isolation_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "tenant_isolation_result",
        tenant_isolation_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(TenantIsolationMetricPlan {
        metric_name: "prodex_tenant_isolation_events_total",
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
        "postgres_tenant_context_operation",
        postgres_tenant_context_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "postgres_tenant_context_result",
        postgres_tenant_context_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(PostgresTenantContextMetricPlan {
        metric_name: "prodex_postgres_tenant_context_events_total",
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
        "identity_context_surface",
        identity_context_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "identity_context_result",
        identity_context_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(IdentityContextMetricPlan {
        metric_name: "prodex_identity_context_events_total",
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
        "break_glass_operation",
        break_glass_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "break_glass_result",
        break_glass_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(BreakGlassLifecycleMetricPlan {
        metric_name: "prodex_break_glass_lifecycle_events_total",
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
        "user_lifecycle_operation",
        user_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "user_lifecycle_result",
        user_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(UserLifecycleMetricPlan {
        metric_name: "prodex_user_lifecycle_events_total",
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
        "service_identity_operation",
        service_identity_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "service_identity_result",
        service_identity_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ServiceIdentityLifecycleMetricPlan {
        metric_name: "prodex_service_identity_lifecycle_events_total",
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
        "role_binding_operation",
        role_binding_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "role_binding_result",
        role_binding_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(RoleBindingLifecycleMetricPlan {
        metric_name: "prodex_role_binding_lifecycle_events_total",
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
        "provider_credential_operation",
        provider_credential_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "provider_credential_result",
        provider_credential_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ProviderCredentialLifecycleMetricPlan {
        metric_name: "prodex_provider_credential_lifecycle_events_total",
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
        "credential_lifecycle_operation",
        virtual_key_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "credential_lifecycle_result",
        virtual_key_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(VirtualKeyLifecycleMetricPlan {
        metric_name: "prodex_virtual_key_lifecycle_events_total",
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
        "budget_policy_operation",
        budget_policy_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "budget_policy_result",
        budget_policy_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(BudgetPolicyLifecycleMetricPlan {
        metric_name: "prodex_budget_policy_lifecycle_events_total",
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
        "policy_lifecycle_operation",
        policy_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "policy_lifecycle_result",
        policy_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(PolicyLifecycleMetricPlan {
        metric_name: "prodex_policy_lifecycle_events_total",
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
        "account_lifecycle_operation",
        tenant_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "account_lifecycle_result",
        tenant_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(TenantLifecycleMetricPlan {
        metric_name: "prodex_tenant_lifecycle_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

fn security_decision_kind_label(decision: SecurityDecisionKind) -> &'static str {
    match decision {
        SecurityDecisionKind::Authentication => "authentication",
        SecurityDecisionKind::TenantResolution => "tenant_resolution",
        SecurityDecisionKind::Authorization => "authorization",
        SecurityDecisionKind::CredentialScope => "credential_scope",
    }
}

fn security_decision_result_label(result: SecurityDecisionResult) -> &'static str {
    match result {
        SecurityDecisionResult::Allowed => "allowed",
        SecurityDecisionResult::Denied => "denied",
        SecurityDecisionResult::Error => "error",
    }
}

fn authn_token_validation_stage_label(stage: AuthnTokenValidationStage) -> &'static str {
    match stage {
        AuthnTokenValidationStage::Decode => "decode",
        AuthnTokenValidationStage::Signature => "signature",
        AuthnTokenValidationStage::Claims => "claims",
        AuthnTokenValidationStage::TenantClaim => "tenant_claim",
        AuthnTokenValidationStage::RoleClaim => "role_claim",
        AuthnTokenValidationStage::JwksCache => "jwks_cache",
    }
}

fn authn_token_validation_result_label(result: AuthnTokenValidationResult) -> &'static str {
    match result {
        AuthnTokenValidationResult::Accepted => "accepted",
        AuthnTokenValidationResult::Malformed => "malformed",
        AuthnTokenValidationResult::InvalidSignature => "invalid_signature",
        AuthnTokenValidationResult::Expired => "expired",
        AuthnTokenValidationResult::UnknownKey => "unknown_key",
        AuthnTokenValidationResult::MissingTenant => "missing_tenant",
        AuthnTokenValidationResult::RoleDenied => "role_denied",
        AuthnTokenValidationResult::CacheUnavailable => "cache_unavailable",
    }
}

fn authz_boundary_kind_label(boundary: AuthzBoundaryKind) -> &'static str {
    match boundary {
        AuthzBoundaryKind::DataPlaneInference => "data_plane_inference",
        AuthzBoundaryKind::DataPlaneQuota => "data_plane_quota",
        AuthzBoundaryKind::ControlPlaneRead => "control_plane_read",
        AuthzBoundaryKind::ControlPlaneMutation => "control_plane_mutation",
        AuthzBoundaryKind::ControlPlaneBilling => "control_plane_billing",
        AuthzBoundaryKind::BreakGlass => "break_glass",
    }
}

fn authz_decision_result_label(result: AuthzDecisionResult) -> &'static str {
    match result {
        AuthzDecisionResult::Allowed => "allowed",
        AuthzDecisionResult::CredentialScopeDenied => "credential_scope_denied",
        AuthzDecisionResult::RoleDenied => "role_denied",
        AuthzDecisionResult::TenantDenied => "tenant_denied",
        AuthzDecisionResult::ResourceDenied => "resource_denied",
        AuthzDecisionResult::Failed => "failed",
    }
}

fn credential_scope_mismatch_direction_label(
    direction: CredentialScopeMismatchDirection,
) -> &'static str {
    match direction {
        CredentialScopeMismatchDirection::DataPlaneToControlPlane => "data_plane_to_control_plane",
        CredentialScopeMismatchDirection::ControlPlaneToDataPlane => "control_plane_to_data_plane",
        CredentialScopeMismatchDirection::BreakGlassToDataPlane => "break_glass_to_data_plane",
        CredentialScopeMismatchDirection::BreakGlassToControlPlane => {
            "break_glass_to_control_plane"
        }
        CredentialScopeMismatchDirection::MissingCredential => "missing_credential",
    }
}

fn credential_scope_mismatch_result_label(result: CredentialScopeMismatchResult) -> &'static str {
    match result {
        CredentialScopeMismatchResult::Rejected => "rejected",
        CredentialScopeMismatchResult::Audited => "audited",
        CredentialScopeMismatchResult::Failed => "failed",
    }
}

fn tenant_isolation_surface_label(surface: TenantIsolationSurface) -> &'static str {
    match surface {
        TenantIsolationSurface::Authentication => "authentication",
        TenantIsolationSurface::Authorization => "authorization",
        TenantIsolationSurface::StoragePredicate => "storage_predicate",
        TenantIsolationSurface::CacheKey => "cache_key",
        TenantIsolationSurface::AuditQuery => "audit_query",
    }
}

fn tenant_isolation_result_label(result: TenantIsolationResult) -> &'static str {
    match result {
        TenantIsolationResult::Enforced => "enforced",
        TenantIsolationResult::CrossTenantDenied => "cross_tenant_denied",
        TenantIsolationResult::MissingTenantDenied => "missing_tenant_denied",
        TenantIsolationResult::MismatchRejected => "mismatch_rejected",
        TenantIsolationResult::Failed => "failed",
    }
}

fn postgres_tenant_context_operation_label(
    operation: PostgresTenantContextOperation,
) -> &'static str {
    match operation {
        PostgresTenantContextOperation::SetContext => "set_context",
        PostgresTenantContextOperation::VerifyContext => "verify_context",
        PostgresTenantContextOperation::ApplyRlsPolicy => "apply_rls_policy",
        PostgresTenantContextOperation::ExecuteTenantDml => "execute_tenant_dml",
    }
}

fn postgres_tenant_context_result_label(result: PostgresTenantContextResult) -> &'static str {
    match result {
        PostgresTenantContextResult::Applied => "applied",
        PostgresTenantContextResult::Missing => "missing",
        PostgresTenantContextResult::MismatchRejected => "mismatch_rejected",
        PostgresTenantContextResult::RlsDenied => "rls_denied",
        PostgresTenantContextResult::Failed => "failed",
    }
}

fn identity_context_surface_label(surface: IdentityContextSurface) -> &'static str {
    match surface {
        IdentityContextSurface::Authentication => "authentication",
        IdentityContextSurface::Authorization => "authorization",
        IdentityContextSurface::Audit => "audit",
        IdentityContextSurface::ControlPlane => "control_plane",
        IdentityContextSurface::DataPlane => "data_plane",
    }
}

fn identity_context_result_label(result: IdentityContextResult) -> &'static str {
    match result {
        IdentityContextResult::Consistent => "consistent",
        IdentityContextResult::MissingPrincipal => "missing_principal",
        IdentityContextResult::MissingTenant => "missing_tenant",
        IdentityContextResult::TenantMismatch => "tenant_mismatch",
        IdentityContextResult::CorrelationMissing => "correlation_missing",
        IdentityContextResult::Failed => "failed",
    }
}

fn break_glass_lifecycle_operation_label(operation: BreakGlassLifecycleOperation) -> &'static str {
    match operation {
        BreakGlassLifecycleOperation::Request => "request",
        BreakGlassLifecycleOperation::Approve => "approve",
        BreakGlassLifecycleOperation::Activate => "activate",
        BreakGlassLifecycleOperation::Revoke => "revoke",
        BreakGlassLifecycleOperation::Expire => "expire",
    }
}

fn break_glass_lifecycle_result_label(result: BreakGlassLifecycleResult) -> &'static str {
    match result {
        BreakGlassLifecycleResult::Authorized => "authorized",
        BreakGlassLifecycleResult::Denied => "denied",
        BreakGlassLifecycleResult::Persisted => "persisted",
        BreakGlassLifecycleResult::Expired => "expired",
        BreakGlassLifecycleResult::Failed => "failed",
    }
}

fn user_lifecycle_operation_label(operation: UserLifecycleOperation) -> &'static str {
    match operation {
        UserLifecycleOperation::Invite => "invite",
        UserLifecycleOperation::ScimCreate => "scim_create",
        UserLifecycleOperation::ScimUpdate => "scim_update",
        UserLifecycleOperation::ScimDelete => "scim_delete",
    }
}

fn user_lifecycle_result_label(result: UserLifecycleResult) -> &'static str {
    match result {
        UserLifecycleResult::Authorized => "authorized",
        UserLifecycleResult::Denied => "denied",
        UserLifecycleResult::Persisted => "persisted",
        UserLifecycleResult::Failed => "failed",
    }
}

fn service_identity_lifecycle_operation_label(
    operation: ServiceIdentityLifecycleOperation,
) -> &'static str {
    match operation {
        ServiceIdentityLifecycleOperation::Create => "create",
        ServiceIdentityLifecycleOperation::RotateSecret => "rotate_secret",
        ServiceIdentityLifecycleOperation::Disable => "disable",
    }
}

fn service_identity_lifecycle_result_label(result: ServiceIdentityLifecycleResult) -> &'static str {
    match result {
        ServiceIdentityLifecycleResult::Authorized => "authorized",
        ServiceIdentityLifecycleResult::Denied => "denied",
        ServiceIdentityLifecycleResult::Persisted => "persisted",
        ServiceIdentityLifecycleResult::Failed => "failed",
    }
}

fn role_binding_lifecycle_operation_label(
    operation: RoleBindingLifecycleOperation,
) -> &'static str {
    match operation {
        RoleBindingLifecycleOperation::Grant => "grant",
        RoleBindingLifecycleOperation::Revoke => "revoke",
    }
}

fn role_binding_lifecycle_result_label(result: RoleBindingLifecycleResult) -> &'static str {
    match result {
        RoleBindingLifecycleResult::Authorized => "authorized",
        RoleBindingLifecycleResult::Denied => "denied",
        RoleBindingLifecycleResult::Persisted => "persisted",
        RoleBindingLifecycleResult::Failed => "failed",
    }
}

fn provider_credential_lifecycle_operation_label(
    operation: ProviderCredentialLifecycleOperation,
) -> &'static str {
    match operation {
        ProviderCredentialLifecycleOperation::Rotate => "rotate",
        ProviderCredentialLifecycleOperation::ValidateReference => "validate_reference",
        ProviderCredentialLifecycleOperation::PersistReference => "persist_reference",
    }
}

fn provider_credential_lifecycle_result_label(
    result: ProviderCredentialLifecycleResult,
) -> &'static str {
    match result {
        ProviderCredentialLifecycleResult::Authorized => "authorized",
        ProviderCredentialLifecycleResult::Denied => "denied",
        ProviderCredentialLifecycleResult::Persisted => "persisted",
        ProviderCredentialLifecycleResult::Failed => "failed",
    }
}

fn virtual_key_lifecycle_operation_label(operation: VirtualKeyLifecycleOperation) -> &'static str {
    match operation {
        VirtualKeyLifecycleOperation::Create => "create",
        VirtualKeyLifecycleOperation::RotateSecret => "rotate_secret",
        VirtualKeyLifecycleOperation::PersistReference => "persist_reference",
    }
}

fn virtual_key_lifecycle_result_label(result: VirtualKeyLifecycleResult) -> &'static str {
    match result {
        VirtualKeyLifecycleResult::Authorized => "authorized",
        VirtualKeyLifecycleResult::Denied => "denied",
        VirtualKeyLifecycleResult::Persisted => "persisted",
        VirtualKeyLifecycleResult::Failed => "failed",
    }
}

fn budget_policy_lifecycle_operation_label(
    operation: BudgetPolicyLifecycleOperation,
) -> &'static str {
    match operation {
        BudgetPolicyLifecycleOperation::Update => "update",
        BudgetPolicyLifecycleOperation::ValidateScope => "validate_scope",
        BudgetPolicyLifecycleOperation::PersistPolicy => "persist_policy",
    }
}

fn budget_policy_lifecycle_result_label(result: BudgetPolicyLifecycleResult) -> &'static str {
    match result {
        BudgetPolicyLifecycleResult::Authorized => "authorized",
        BudgetPolicyLifecycleResult::Denied => "denied",
        BudgetPolicyLifecycleResult::Persisted => "persisted",
        BudgetPolicyLifecycleResult::Failed => "failed",
    }
}

fn policy_lifecycle_operation_label(operation: PolicyLifecycleOperation) -> &'static str {
    match operation {
        PolicyLifecycleOperation::Create => "create",
        PolicyLifecycleOperation::Update => "update",
        PolicyLifecycleOperation::Publish => "publish",
        PolicyLifecycleOperation::Invalidate => "invalidate",
    }
}

fn policy_lifecycle_result_label(result: PolicyLifecycleResult) -> &'static str {
    match result {
        PolicyLifecycleResult::Authorized => "authorized",
        PolicyLifecycleResult::Denied => "denied",
        PolicyLifecycleResult::Persisted => "persisted",
        PolicyLifecycleResult::Published => "published",
        PolicyLifecycleResult::Failed => "failed",
    }
}

fn tenant_lifecycle_operation_label(operation: TenantLifecycleOperation) -> &'static str {
    match operation {
        TenantLifecycleOperation::Create => "create",
        TenantLifecycleOperation::Update => "update",
    }
}

fn tenant_lifecycle_result_label(result: TenantLifecycleResult) -> &'static str {
    match result {
        TenantLifecycleResult::Authorized => "authorized",
        TenantLifecycleResult::Denied => "denied",
        TenantLifecycleResult::Persisted => "persisted",
        TenantLifecycleResult::Failed => "failed",
    }
}
