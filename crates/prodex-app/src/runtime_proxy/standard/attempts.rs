use super::*;

#[path = "attempts/compact.rs"]
mod compact;
#[path = "attempts/models.rs"]
mod models;
#[path = "attempts/noncompact.rs"]
mod noncompact;
#[path = "attempts/precommit.rs"]
mod precommit;
#[path = "attempts/success.rs"]
mod success;
use models::forward_runtime_standard_success_response;
use precommit::{
    RuntimeStandardPrecommitGuard, runtime_compact_precommit_quota_guard,
    runtime_standard_precommit_quota_guard,
};
use success::forward_runtime_compact_success_attempt;

fn handle_runtime_standard_upstream_error(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    stage: &'static str,
    err: anyhow::Error,
) -> Result<RuntimeStandardAttempt> {
    note_runtime_profile_transport_failure(shared, profile_name, route_kind, stage, &err);
    if is_runtime_proxy_transport_failure(&err) {
        return Ok(RuntimeStandardAttempt::TransportFailed {
            profile_name: profile_name.to_string(),
            stage,
        });
    }
    Err(err)
}

pub(super) use compact::attempt_runtime_standard_request;
pub(super) use noncompact::{
    attempt_runtime_noncompact_standard_request,
    attempt_runtime_noncompact_standard_request_with_policy,
};
