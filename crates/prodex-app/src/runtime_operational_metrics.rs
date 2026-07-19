use prodex_application::{
    ApplicationAuthorizedRequestContext, ApplicationRequestAuthorizationError,
};
use prodex_authn::{AuthenticationError, VerifiedCredentialAuthenticationError};
use prodex_domain::TelemetryAttribute;
use prodex_gateway_http::{GatewayHttpMethod, GatewayHttpRequestMeta};
use prodex_observability::{
    AuthnTokenValidationResult, AuthnTokenValidationStage, AuthzBoundaryKind, AuthzDecisionResult,
    PolicyLifecycleOperation, PolicyLifecycleResult, SecretProviderBackend,
    SecretProviderOperation, SecretProviderResult, TenantIsolationResult, TenantIsolationSurface,
    plan_authn_token_validation_metric, plan_authz_decision_metric, plan_policy_lifecycle_metric,
    plan_secret_provider_metric, plan_tenant_isolation_metric,
};
use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RuntimeOperationalMetricKey {
    name: &'static str,
    labels: Vec<(String, String)>,
}

#[derive(Default)]
struct RuntimeOperationalMetricRegistry {
    counters: Mutex<BTreeMap<RuntimeOperationalMetricKey, u64>>,
}

static RUNTIME_OPERATIONAL_METRICS: LazyLock<RuntimeOperationalMetricRegistry> =
    LazyLock::new(RuntimeOperationalMetricRegistry::default);

impl RuntimeOperationalMetricRegistry {
    fn record(&self, name: &'static str, increment: u64, labels: &[&TelemetryAttribute]) {
        let mut metric_labels = Vec::with_capacity(labels.len());
        for label in labels {
            let Ok((key, value)) = label.as_metric_label() else {
                return;
            };
            metric_labels.push((key.to_string(), value.to_string()));
        }
        metric_labels.sort();
        let key = RuntimeOperationalMetricKey {
            name,
            labels: metric_labels,
        };
        if let Ok(mut counters) = self.counters.lock() {
            let counter = counters.entry(key).or_default();
            *counter = counter.saturating_add(increment);
        }
    }

    fn render(&self) -> String {
        let Ok(counters) = self.counters.lock() else {
            return String::new();
        };
        render_runtime_operational_metrics(&counters)
    }
}

pub(crate) fn record_runtime_authn_metric(
    stage: AuthnTokenValidationStage,
    result: AuthnTokenValidationResult,
) {
    let Ok(plan) = plan_authn_token_validation_metric(stage, result) else {
        return;
    };
    RUNTIME_OPERATIONAL_METRICS.record(
        plan.metric_name,
        plan.increment,
        &[&plan.stage_label, &plan.result_label],
    );
}

pub(crate) fn record_runtime_authentication_error(error: &VerifiedCredentialAuthenticationError) {
    let (stage, result) = match error {
        VerifiedCredentialAuthenticationError::Oidc(AuthenticationError::SignatureNotVerified) => (
            AuthnTokenValidationStage::Signature,
            AuthnTokenValidationResult::InvalidSignature,
        ),
        VerifiedCredentialAuthenticationError::Oidc(AuthenticationError::UnknownKeyId) => (
            AuthnTokenValidationStage::Signature,
            AuthnTokenValidationResult::UnknownKey,
        ),
        VerifiedCredentialAuthenticationError::Oidc(AuthenticationError::TokenExpired) => (
            AuthnTokenValidationStage::Claims,
            AuthnTokenValidationResult::Expired,
        ),
        VerifiedCredentialAuthenticationError::Oidc(AuthenticationError::MissingTenant) => (
            AuthnTokenValidationStage::TenantClaim,
            AuthnTokenValidationResult::MissingTenant,
        ),
        VerifiedCredentialAuthenticationError::Oidc(AuthenticationError::Role(_)) => (
            AuthnTokenValidationStage::RoleClaim,
            AuthnTokenValidationResult::RoleDenied,
        ),
        VerifiedCredentialAuthenticationError::Oidc(
            AuthenticationError::JwksRefreshRequired
            | AuthenticationError::JwksUnavailable
            | AuthenticationError::JwksRefreshForbiddenOnRequestPath
            | AuthenticationError::InvalidJwksUrl
            | AuthenticationError::JwksUrlIssuerMismatch,
        ) => (
            AuthnTokenValidationStage::JwksCache,
            AuthnTokenValidationResult::CacheUnavailable,
        ),
        VerifiedCredentialAuthenticationError::CredentialRequired => (
            AuthnTokenValidationStage::Decode,
            AuthnTokenValidationResult::Malformed,
        ),
        VerifiedCredentialAuthenticationError::CredentialScopeMismatch { .. }
        | VerifiedCredentialAuthenticationError::Oidc(AuthenticationError::TokenNotYetValid)
        | VerifiedCredentialAuthenticationError::Oidc(AuthenticationError::Claims(_))
        | VerifiedCredentialAuthenticationError::OidcPrincipalMismatch
        | VerifiedCredentialAuthenticationError::WorkloadIdentityMismatch
        | VerifiedCredentialAuthenticationError::WorkloadMtlsRequired => (
            AuthnTokenValidationStage::Claims,
            AuthnTokenValidationResult::Malformed,
        ),
    };
    record_runtime_authn_metric(stage, result);
}

pub(crate) fn record_runtime_authz_metric(
    boundary: AuthzBoundaryKind,
    result: AuthzDecisionResult,
) {
    let Ok(plan) = plan_authz_decision_metric(boundary, result) else {
        return;
    };
    RUNTIME_OPERATIONAL_METRICS.record(
        plan.metric_name,
        plan.increment,
        &[&plan.boundary_label, &plan.result_label],
    );
}

pub(crate) fn record_runtime_authorization(
    boundary: AuthzBoundaryKind,
    result: &Result<ApplicationAuthorizedRequestContext<'_>, ApplicationRequestAuthorizationError>,
) {
    let authz_result = match result {
        Ok(_) => AuthzDecisionResult::Allowed,
        Err(ApplicationRequestAuthorizationError::WrongPlane) => {
            AuthzDecisionResult::CredentialScopeDenied
        }
        Err(ApplicationRequestAuthorizationError::AnonymousNotAllowed) => {
            AuthzDecisionResult::RoleDenied
        }
        Err(ApplicationRequestAuthorizationError::PrincipalMismatch) => {
            AuthzDecisionResult::ResourceDenied
        }
        Err(ApplicationRequestAuthorizationError::Tenant(_)) => AuthzDecisionResult::TenantDenied,
        Err(ApplicationRequestAuthorizationError::DataPlane(error)) => match error {
            prodex_authz::BoundaryAuthorizationError::CredentialScopeMismatch { .. } => {
                AuthzDecisionResult::CredentialScopeDenied
            }
            prodex_authz::BoundaryAuthorizationError::InsufficientRole { .. } => {
                AuthzDecisionResult::RoleDenied
            }
            prodex_authz::BoundaryAuthorizationError::PrincipalKindMismatch { .. } => {
                AuthzDecisionResult::ResourceDenied
            }
            prodex_authz::BoundaryAuthorizationError::Tenant(_) => {
                AuthzDecisionResult::TenantDenied
            }
        },
        Err(ApplicationRequestAuthorizationError::ControlPlane(error)) => match error {
            prodex_control_plane::ControlPlaneAuthorizationError::CredentialScopeMismatch {
                ..
            } => AuthzDecisionResult::CredentialScopeDenied,
            prodex_control_plane::ControlPlaneAuthorizationError::InsufficientRole { .. } => {
                AuthzDecisionResult::RoleDenied
            }
            prodex_control_plane::ControlPlaneAuthorizationError::Tenant(_) => {
                AuthzDecisionResult::TenantDenied
            }
            prodex_control_plane::ControlPlaneAuthorizationError::ResourceKindMismatch { .. }
            | prodex_control_plane::ControlPlaneAuthorizationError::BreakGlassExpired { .. }
            | prodex_control_plane::ControlPlaneAuthorizationError::BreakGlassPrincipalKindMismatch {
                ..
            }
            | prodex_control_plane::ControlPlaneAuthorizationError::BreakGlassReasonMissing
            | prodex_control_plane::ControlPlaneAuthorizationError::BreakGlassReasonMalformed => {
                AuthzDecisionResult::ResourceDenied
            }
        },
    };
    record_runtime_authz_metric(boundary, authz_result);

    match result {
        Ok(authorized) if authorized.tenant_context().is_some() => {
            record_runtime_tenant_isolation_metric(
                TenantIsolationSurface::Authorization,
                TenantIsolationResult::Enforced,
            );
        }
        Err(ApplicationRequestAuthorizationError::Tenant(_)) => {
            record_runtime_tenant_isolation_metric(
                TenantIsolationSurface::Authorization,
                TenantIsolationResult::MissingTenantDenied,
            );
        }
        Err(ApplicationRequestAuthorizationError::DataPlane(
            prodex_authz::BoundaryAuthorizationError::Tenant(error),
        ))
        | Err(ApplicationRequestAuthorizationError::ControlPlane(
            prodex_control_plane::ControlPlaneAuthorizationError::Tenant(error),
        )) => {
            let result = match error {
                prodex_domain::TenantAccessError::PrincipalMissingTenant => {
                    TenantIsolationResult::MissingTenantDenied
                }
                prodex_domain::TenantAccessError::CrossTenantAccess { .. } => {
                    TenantIsolationResult::CrossTenantDenied
                }
            };
            record_runtime_tenant_isolation_metric(TenantIsolationSurface::Authorization, result);
        }
        _ => {}
    }
}

pub(crate) fn runtime_control_plane_authz_boundary(
    http: &GatewayHttpRequestMeta,
) -> AuthzBoundaryKind {
    if http.path.contains("/billing") {
        AuthzBoundaryKind::ControlPlaneBilling
    } else if matches!(
        http.method,
        GatewayHttpMethod::Get | GatewayHttpMethod::Options
    ) {
        AuthzBoundaryKind::ControlPlaneRead
    } else {
        AuthzBoundaryKind::ControlPlaneMutation
    }
}

pub(crate) fn record_runtime_tenant_isolation_metric(
    surface: TenantIsolationSurface,
    result: TenantIsolationResult,
) {
    let Ok(plan) = plan_tenant_isolation_metric(surface, result) else {
        return;
    };
    RUNTIME_OPERATIONAL_METRICS.record(
        plan.metric_name,
        plan.increment,
        &[&plan.surface_label, &plan.result_label],
    );
}

pub(crate) fn record_runtime_policy_lifecycle_metric(
    operation: PolicyLifecycleOperation,
    result: PolicyLifecycleResult,
) {
    let Ok(plan) = plan_policy_lifecycle_metric(operation, result) else {
        return;
    };
    RUNTIME_OPERATIONAL_METRICS.record(
        plan.metric_name,
        plan.increment,
        &[&plan.operation_label, &plan.result_label],
    );
}

pub(crate) fn record_runtime_secret_provider_metric(
    backend: SecretProviderBackend,
    operation: SecretProviderOperation,
    result: SecretProviderResult,
) {
    let Ok(plan) = plan_secret_provider_metric(backend, operation, result) else {
        return;
    };
    RUNTIME_OPERATIONAL_METRICS.record(
        plan.metric_name,
        plan.increment,
        &[
            &plan.backend_label,
            &plan.operation_label,
            &plan.result_label,
        ],
    );
}

pub(crate) fn runtime_operational_prometheus_text() -> String {
    RUNTIME_OPERATIONAL_METRICS.render()
}

fn render_runtime_operational_metrics(
    counters: &BTreeMap<RuntimeOperationalMetricKey, u64>,
) -> String {
    let mut body = String::new();
    let mut previous_name = None;
    for (key, value) in counters {
        if previous_name != Some(key.name) {
            body.push_str("# TYPE ");
            body.push_str(key.name);
            body.push_str(" counter\n");
            previous_name = Some(key.name);
        }
        body.push_str(key.name);
        if !key.labels.is_empty() {
            body.push('{');
            for (index, (label, value)) in key.labels.iter().enumerate() {
                if index > 0 {
                    body.push(',');
                }
                body.push_str(label);
                body.push_str("=\"");
                push_prometheus_label_value(&mut body, value);
                body.push('"');
            }
            body.push('}');
        }
        body.push(' ');
        body.push_str(&value.to_string());
        body.push('\n');
    }
    body
}

fn push_prometheus_label_value(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            character => output.push(character),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_renders_closed_labels_as_prometheus_counters() {
        let registry = RuntimeOperationalMetricRegistry::default();
        let plan = plan_authz_decision_metric(
            AuthzBoundaryKind::DataPlaneInference,
            AuthzDecisionResult::Allowed,
        )
        .unwrap();
        registry.record(
            plan.metric_name,
            plan.increment,
            &[&plan.boundary_label, &plan.result_label],
        );

        let rendered = registry.render();
        assert!(rendered.contains("# TYPE prodex_authz_decisions_total counter"));
        assert!(rendered.contains("authz_boundary=\"data_plane_inference\""));
        assert!(rendered.contains("authz_result=\"allowed\""));
    }
}
