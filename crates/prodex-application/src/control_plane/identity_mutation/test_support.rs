use std::fmt::Debug;
use std::str::FromStr;

use prodex_control_plane::{
    ControlPlaneActionPlan, ControlPlaneActionRequest, ControlPlaneDecision, ControlPlaneOperation,
    ControlPlaneResourceRef, decide_control_plane_action,
};
use prodex_domain::{
    CredentialScope, Principal, PrincipalId, PrincipalKind, ResourceKind, Role, TenantId,
};

pub(super) fn id<T>(value: &str) -> T
where
    T: FromStr,
    T::Err: Debug,
{
    value.parse().unwrap()
}

pub(super) fn tenant_id() -> TenantId {
    id("00000000-0000-7000-8000-000000000101")
}

pub(super) fn action_plan(
    operation: ControlPlaneOperation,
    resource_kind: ResourceKind,
    resource_id: Option<&str>,
) -> ControlPlaneActionPlan {
    let tenant_id = tenant_id();
    let principal = Principal::new(
        id::<PrincipalId>("00000000-0000-7000-8000-000000000103"),
        Some(tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    );
    match decide_control_plane_action(ControlPlaneActionRequest {
        principal,
        operation,
        resource: ControlPlaneResourceRef::new(tenant_id, resource_kind, resource_id),
        occurred_at_unix_ms: 1_000,
    }) {
        ControlPlaneDecision::Authorized(plan) => plan,
        ControlPlaneDecision::Denied { .. } => panic!("test action should authorize"),
    }
}
