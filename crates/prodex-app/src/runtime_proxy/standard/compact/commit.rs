use super::super::super::{commit_runtime_proxy_profile_selection_with_notice, runtime_proxy_log};
use crate::runtime_state_shared::{RuntimeRotationProxyShared, RuntimeRouteKind};
use anyhow::Result;

pub(super) fn commit_runtime_proxy_compact_success(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: String,
    response: tiny_http::ResponseBox,
) -> Result<tiny_http::ResponseBox> {
    commit_runtime_proxy_profile_selection_with_notice(
        shared,
        &profile_name,
        RuntimeRouteKind::Compact,
    )?;
    runtime_proxy_log(
        shared,
        format!("request={request_id} transport=http compact_committed profile={profile_name}"),
    );
    Ok(response)
}
