use super::local_rewrite_application_boundary::runtime_gateway_stable_id;
use super::local_rewrite_application_data_plane::RuntimeGatewayApplicationAdmission;
use prodex_domain::{ApprovalId, ApprovalState, TenantId};
use prodex_provider_core::ProviderModelCost;

pub(super) const RUNTIME_GATEWAY_REALTIME_SESSION_MAX_TOKENS: u64 = 32_768;
pub(super) const RUNTIME_GATEWAY_REALTIME_SESSION_MAX_MILLIS: u64 = 5 * 60 * 1_000;
pub(super) const RUNTIME_GATEWAY_REALTIME_FRAME_MAX_BYTES: usize = 32 * 1_024;

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
