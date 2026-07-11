use super::*;
use std::collections::BTreeSet;

mod discovery;

use discovery::discover_runtime_previous_response_candidate;

#[cfg(any(test, feature = "bench-support"))]
pub(crate) fn next_runtime_previous_response_candidate(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let mut trace = runtime_selection_trace_builder(route_kind, None);
    next_runtime_previous_response_candidate_with_trace(
        shared,
        excluded_profiles,
        previous_response_id,
        route_kind,
        &mut trace,
    )
}

pub(super) fn next_runtime_previous_response_candidate_with_trace(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
) -> Result<Option<String>> {
    discover_runtime_previous_response_candidate(
        shared,
        excluded_profiles,
        previous_response_id,
        route_kind,
        trace,
    )
}
