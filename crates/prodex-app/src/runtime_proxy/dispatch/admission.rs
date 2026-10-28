//! Translate admission failures without changing retry or quota policy.
use super::{
    RuntimeProxyAdmissionRejection, RuntimeRotationProxyShared, mark_runtime_proxy_local_overload,
    reject_runtime_proxy_overloaded_request, reject_runtime_proxy_recovery_timeout,
    runtime_proxy_dispatch_admission_plan, runtime_route_kind_label,
};

pub(super) fn reject_runtime_proxy_admission_rejection(
    request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
    rejection: RuntimeProxyAdmissionRejection,
) {
    let plan = runtime_proxy_dispatch_admission_plan(rejection, false, true);
    debug_assert!(matches!(
        plan.action,
        prodex_mojo_core::runtime_proxy_dispatch::RuntimeProxyAdmissionAction::Reject
    ));
    match rejection {
        RuntimeProxyAdmissionRejection::DeadlineExceeded => {
            reject_runtime_proxy_recovery_timeout(request, shared, "admission");
        }
        RuntimeProxyAdmissionRejection::GlobalLimit => {
            if plan.marks_global_overload {
                mark_runtime_proxy_local_overload(shared, "active_request_limit");
            }
            reject_runtime_proxy_overloaded_request(request, shared, "active_request_limit");
        }
        RuntimeProxyAdmissionRejection::LaneLimit(lane) => {
            let reason = format!("lane_limit:{}", runtime_route_kind_label(lane));
            if plan.marks_global_overload {
                mark_runtime_proxy_local_overload(shared, &reason);
            }
            reject_runtime_proxy_overloaded_request(request, shared, &reason);
        }
    }
}
