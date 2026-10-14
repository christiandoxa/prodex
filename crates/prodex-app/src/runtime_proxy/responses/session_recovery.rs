use super::*;

/// Requests carrying all history and no opaque continuation may safely start a
/// new session routing epoch. A response/turn token or committed output never
/// takes this path, even if other accounts advertise available quota.
pub(super) fn reset_stale_session_conflict_for_full_history(
    shared: &RuntimeRotationProxyShared,
    request: &RuntimeProxyRequest,
    request_id: u64,
    previous_response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<()> {
    let Some(session_id) = session_id else {
        return Ok(());
    };
    if previous_response_id.is_some() || turn_state.is_some() {
        return Ok(());
    }
    let owner = runtime_request_hard_binding_owner(shared, None, None, Some(session_id))?;
    let conflict = owner == prodex_runtime_state::RuntimeHardBindingOwner::Conflict;
    if !conflict {
        return Ok(());
    }
    let allowed = prodex_mojo_core::runtime_lineage::session_full_history_reset_allowed(
        previous_response_id.is_some(),
        turn_state.is_some(),
        runtime_proxy_crate::runtime_request_has_reconstructable_full_history(request),
        conflict,
        false,
    )
    .map_err(|error| anyhow::anyhow!("Mojo full-history session reset policy failed: {error:?}"))?;
    if allowed
        && release_runtime_session_affinity(shared, session_id, "full_history_conflict_recovery")?
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} session_conflict_recovered full_history=true replayed_opaque_state=false"
            ),
        );
    }
    Ok(())
}
