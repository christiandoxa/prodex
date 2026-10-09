use super::super::*;

/// Fallible adapters for the selection kernels.
///
/// The runtime owns state reads and side effects; Mojo owns the precedence and
/// eligibility decisions. Keeping the error at this boundary prevents a bad
/// ABI result from becoming a fresh-profile fallback.
pub(crate) fn runtime_affinity_selection_plan(
    input: prodex_mojo_core::runtime::AffinitySelectionInput<'_>,
) -> Result<prodex_mojo_core::runtime::AffinitySelectionPlan> {
    prodex_mojo_core::runtime::affinity_selection_plan(input)
        .map_err(|error| anyhow::anyhow!("Mojo affinity selection planning failed: {error:?}"))
}

pub(crate) fn runtime_affinity_outcome(
    input: runtime_proxy_crate::RuntimeAffinityOutcomeInput,
) -> Result<runtime_proxy_crate::RuntimeAffinityOutcome> {
    runtime_proxy_crate::runtime_affinity_outcome_checked(input)
        .map_err(|error| anyhow::anyhow!("Mojo affinity outcome planning failed: {error:?}"))
}

pub(crate) fn runtime_soft_affinity_allowed(
    input: runtime_proxy_crate::RuntimeSoftAffinityPolicyInput,
) -> Result<bool> {
    runtime_proxy_crate::runtime_soft_affinity_allowed_checked(input)
        .map_err(|error| anyhow::anyhow!("Mojo soft-affinity planning failed: {error:?}"))
}

pub(crate) fn runtime_soft_affinity_rejection_reason(
    input: runtime_proxy_crate::RuntimeSoftAffinityPolicyInput,
) -> Result<&'static str> {
    runtime_proxy_crate::runtime_soft_affinity_rejection_reason_checked(input)
        .map_err(|error| anyhow::anyhow!("Mojo soft-affinity reason failed: {error:?}"))
}

pub(crate) fn runtime_waitable_candidate_eligible(
    mode: prodex_mojo_core::runtime::WaitableCandidateMode,
    input: prodex_mojo_core::runtime::WaitableCandidateInput,
) -> Result<bool> {
    prodex_mojo_core::runtime::waitable_candidate_eligible(mode, input)
        .map_err(|error| anyhow::anyhow!("Mojo waitable-candidate planning failed: {error:?}"))
}

pub(crate) fn runtime_profile_selection_backoff_active(
    retry_until: Option<i64>,
    transport_until: Option<i64>,
    circuit_until: Option<i64>,
    now: i64,
) -> Result<bool> {
    prodex_mojo_core::runtime::profile_selection_backoff_active(
        retry_until,
        transport_until,
        circuit_until,
        now,
    )
    .map_err(|error| anyhow::anyhow!("Mojo selection-backoff planning failed: {error:?}"))
}
