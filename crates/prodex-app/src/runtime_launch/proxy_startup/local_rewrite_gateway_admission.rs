use super::local_rewrite_application_boundary::runtime_gateway_stable_id;
use super::local_rewrite_application_data_plane::{
    RuntimeGatewayApplicationAdmission, RuntimeGatewayApplicationDataPlaneError,
    runtime_gateway_application_data_plane_admission, runtime_gateway_application_http_policy,
};
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis;
use crate::{RuntimeProxyRequest, runtime_proxy_log};
use prodex_application::ApplicationInspectionPlan;
use prodex_domain::{
    ApprovalId, ApprovalState, BudgetLimit, BudgetSnapshot, CallId, IdempotencyKey,
    PrincipalPolicyAttributes, ReservationRequest, TenantId, UsageAmount,
};
use prodex_provider_core::ProviderModelCost;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};

pub(super) const RUNTIME_GATEWAY_REALTIME_SESSION_MAX_TOKENS: u64 = 32_768;
pub(super) const RUNTIME_GATEWAY_REALTIME_SESSION_MAX_MILLIS: u64 = 5 * 60 * 1_000;
pub(super) const RUNTIME_GATEWAY_REALTIME_FRAME_MAX_BYTES: usize = 32 * 1_024;
const RUNTIME_GATEWAY_RESERVATION_COMPLETION_GRACE_MS: u64 = 30_000;

#[derive(Clone)]
pub(super) struct RuntimeGatewayRealtimeAccountingPlan {
    pub(super) token_limit: u64,
    pub(super) model: String,
    pub(super) cost: ProviderModelCost,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct RuntimeGatewayRealtimeUsage {
    pub(super) input_tokens: u64,
    pub(super) output_tokens: u64,
    pub(super) input_bytes: usize,
    pub(super) output_bytes: usize,
    pub(super) policy_interrupted: bool,
}

pub(super) fn runtime_gateway_reservation_ttl_ms(
    shared: &super::local_rewrite::RuntimeLocalRewriteProxyShared,
    realtime: bool,
) -> u64 {
    let request_timeout_ms = runtime_gateway_application_http_policy(shared).request_timeout_ms;
    runtime_gateway_reservation_ttl_for_timeout(request_timeout_ms, realtime)
}

fn runtime_gateway_reservation_ttl_for_timeout(request_timeout_ms: u64, realtime: bool) -> u64 {
    let active_request_limit_ms = if realtime {
        request_timeout_ms.max(RUNTIME_GATEWAY_REALTIME_SESSION_MAX_MILLIS)
    } else {
        request_timeout_ms
    };
    active_request_limit_ms.saturating_add(RUNTIME_GATEWAY_RESERVATION_COMPLETION_GRACE_MS)
}

pub(super) fn runtime_gateway_conversation_namespace(
    tenant_id: &TenantId,
    identity_kind: &str,
    identity: &str,
) -> String {
    let tenant_id = tenant_id.to_string();
    runtime_gateway_stable_id(
        "prodex:gateway-conversation:v1",
        &[
            tenant_id.as_bytes(),
            identity_kind.as_bytes(),
            identity.as_bytes(),
        ],
    )
    .to_string()
}

pub(super) struct RuntimeGatewayVirtualKeyAdmissionOutcome {
    pub(super) namespace: Option<String>,
    pub(super) application: RuntimeGatewayApplicationAdmission,
    pub(super) realtime_accounting: Option<RuntimeGatewayRealtimeAccountingPlan>,
}

pub(super) struct RuntimeGatewayVirtualKeyAdmissionFailure {
    pub(super) rejection: runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection,
    pub(super) approval: Option<(ApprovalId, ApprovalState)>,
}

impl From<runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection>
    for RuntimeGatewayVirtualKeyAdmissionFailure
{
    fn from(rejection: runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection) -> Self {
        Self {
            rejection,
            approval: None,
        }
    }
}

pub(super) fn runtime_gateway_application_admission_rejection(
    error: RuntimeGatewayApplicationDataPlaneError,
) -> RuntimeGatewayVirtualKeyAdmissionFailure {
    match error {
        RuntimeGatewayApplicationDataPlaneError::GovernanceDenied => {
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::GovernanceDenied.into()
        }
        RuntimeGatewayApplicationDataPlaneError::GovernanceApprovalRequired {
            approval_id,
            state,
        } => RuntimeGatewayVirtualKeyAdmissionFailure {
            rejection:
                runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::GovernanceApprovalRequired,
            approval: Some((approval_id, state)),
        },
        RuntimeGatewayApplicationDataPlaneError::GovernanceSessionRequired => {
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::GovernanceSessionRequired.into()
        }
        RuntimeGatewayApplicationDataPlaneError::NoEligibleProvider => {
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::NoEligibleProvider.into()
        }
        RuntimeGatewayApplicationDataPlaneError::Execution(_)
        | RuntimeGatewayApplicationDataPlaneError::MissingPrincipal
        | RuntimeGatewayApplicationDataPlaneError::RouteUnavailable
        | RuntimeGatewayApplicationDataPlaneError::ProviderRoute(_)
        | RuntimeGatewayApplicationDataPlaneError::TraceContext(_)
        | RuntimeGatewayApplicationDataPlaneError::Admission(_)
        | RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable => {
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable.into()
        }
    }
}

pub(super) fn runtime_gateway_application_admission_without_virtual_key(
    request_id: u64,
    captured: &RuntimeProxyRequest,
    shared: &super::local_rewrite::RuntimeLocalRewriteProxyShared,
    network_zone: prodex_domain::NetworkZone,
    authorized: &prodex_application::ApplicationAuthorizedRequestContext<'_>,
    inspection: &ApplicationInspectionPlan,
) -> Result<RuntimeGatewayVirtualKeyAdmissionOutcome, RuntimeGatewayVirtualKeyAdmissionFailure> {
    let realtime_accounting = runtime_proxy_crate::is_runtime_realtime_websocket_path(
        runtime_proxy_crate::path_without_query(&captured.path_and_query),
    )
    .then(|| RuntimeGatewayRealtimeAccountingPlan {
        token_limit: RUNTIME_GATEWAY_REALTIME_SESSION_MAX_TOKENS,
        model: shared
            .runtime_shared
            .runtime_config
            .gemini
            .live_model
            .clone()
            .unwrap_or_else(|| {
                super::local_rewrite_gemini_live::runtime_gemini_live_default_model().to_string()
            }),
        cost: ProviderModelCost::default(),
    });
    let Some(tenant) = authorized.tenant_context() else {
        return Ok(RuntimeGatewayVirtualKeyAdmissionOutcome {
            namespace: None,
            application: RuntimeGatewayApplicationAdmission::compatibility_anonymous(
                authorized.request().route(),
                captured,
                shared,
                inspection.clone(),
            )
            .map_err(|_| {
                RuntimeGatewayVirtualKeyAdmissionFailure::from(
                    runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable,
                )
            })?,
            realtime_accounting,
        });
    };
    let call_id = CallId::new();
    let reservation_id = prodex_domain::ReservationId::new();
    let estimate = UsageAmount::new(
        realtime_accounting
            .as_ref()
            .map(|accounting| accounting.token_limit)
            .unwrap_or_else(|| {
                runtime_proxy_crate::runtime_gateway_estimated_tokens(&captured.body).max(1)
            }),
        0,
    );
    let command = prodex_storage::AtomicReservationCommand {
        storage_key: prodex_storage::TenantStorageKey::tenant(tenant.tenant_id),
        idempotency_key: IdempotencyKey::from_call_reservation(call_id, reservation_id),
        snapshot: BudgetSnapshot::default(),
        limit: BudgetLimit::new(u64::MAX, u64::MAX),
        request: ReservationRequest {
            tenant_id: tenant.tenant_id,
            call_id,
            reservation_id,
            estimate,
        },
        created_at_unix_ms: runtime_gateway_unix_epoch_millis(),
        ttl_ms: runtime_gateway_reservation_ttl_ms(shared, realtime_accounting.is_some()),
    };
    let application = runtime_gateway_application_data_plane_admission(
        authorized,
        captured,
        shared,
        network_zone,
        PrincipalPolicyAttributes::default(),
        command,
        inspection.clone(),
    )
    .map_err(|error| {
        let rejection = runtime_gateway_application_admission_rejection(error);
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_application_admission_failed",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("reason", rejection.rejection.code()),
                ],
            ),
        );
        rejection
    })?;
    let principal_id = authorized
        .principal()
        .map(|principal| principal.id.to_string())
        .unwrap_or_default();
    Ok(RuntimeGatewayVirtualKeyAdmissionOutcome {
        namespace: Some(runtime_gateway_conversation_namespace(
            &tenant.tenant_id,
            "principal",
            &principal_id,
        )),
        application,
        realtime_accounting,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reservation_ttl_covers_request_lifetime_and_completion_grace() {
        assert_eq!(
            runtime_gateway_reservation_ttl_for_timeout(300_000, false),
            330_000
        );
        assert_eq!(
            runtime_gateway_reservation_ttl_for_timeout(600_000, false),
            630_000
        );
        assert_eq!(
            runtime_gateway_reservation_ttl_for_timeout(120_000, true),
            330_000
        );
    }
}
