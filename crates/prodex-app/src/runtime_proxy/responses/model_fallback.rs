use super::{
    RuntimePrecommitLoopState, RuntimeResponseCandidateSelection, RuntimeResponsesAffinityState,
    RuntimeResponsesRequestContext, RuntimeRouteKind, RuntimeUpstreamFailureResponse,
    select_runtime_response_candidate_for_route_with_request,
};
use crate::runtime_proxy_log;
use anyhow::Result;
use std::collections::BTreeSet;

pub(super) fn try_runtime_responses_luna_spark_fallback(
    context: &mut RuntimeResponsesRequestContext<'_>,
    affinity_state: &RuntimeResponsesAffinityState,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
    quota_last_chance_profile: &mut Option<String>,
) -> Result<bool> {
    let Some(spark_model) = prodex_quota::openai_luna_spark_fallback_model(
        context.requested_model_name.as_deref(),
        context.request_model_name.as_deref(),
    ) else {
        return Ok(false);
    };
    if context.previous_response_id.is_some()
        || context.request_turn_state.is_some()
        || context.request_session_id.is_some()
        || context.request_requires_previous_response_affinity
        || context.prompt_cache_key.is_some()
        || affinity_state
            .has_continuation_priority(context.previous_response_id, context.request_turn_state)
        || loop_state.saw_inflight_saturation
    {
        return Ok(false);
    }

    let spark_candidate = select_runtime_response_candidate_for_route_with_request(
        context.shared,
        RuntimeResponseCandidateSelection::fresh(&BTreeSet::new(), RuntimeRouteKind::Responses),
        Some(context.request_id),
        Some(spark_model),
    )?;
    let Some(spark_candidate) = spark_candidate else {
        return Ok(false);
    };

    context.request.body =
        prodex_provider_core::provider_request_body_with_model(&context.request.body, spark_model);
    context.request_model_name = Some(spark_model.to_string());
    loop_state.reset_for_model_fallback();
    *quota_last_chance_profile = Some(spark_candidate.clone());
    runtime_proxy_log(
        context.shared,
        format!(
            "request={} transport=http model_fallback requested_model=luna effective_model={} profile={} reason=luna_capacity_unavailable_spark_available",
            context.request_id, spark_model, spark_candidate
        ),
    );
    Ok(true)
}
