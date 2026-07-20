use super::local_rewrite_application_boundary::runtime_gateway_stable_id;
use super::local_rewrite_application_data_plane::RuntimeGatewayApplicationAdmission;
use prodex_domain::{ApprovalId, ApprovalState, TenantId};

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
