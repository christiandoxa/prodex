use super::{
    RuntimeCompactSelectionContext, RuntimeResponseCandidateSelection, RuntimeRouteKind,
    select_runtime_response_candidate_for_route_with_request,
};
use crate::runtime_proxy_log;
use anyhow::Result;
use std::collections::BTreeSet;
use std::time::Instant;

impl RuntimeCompactSelectionContext<'_> {
    pub(super) fn try_luna_spark_fallback(&mut self) -> Result<bool> {
        let Some(spark_model) = prodex_quota::openai_luna_spark_fallback_model(
            self.requested_model_name.as_deref(),
            self.request_model_name.as_deref(),
        ) else {
            return Ok(false);
        };
        if !self.is_fresh_request() {
            return Ok(false);
        }

        let Some(spark_candidate) = select_runtime_response_candidate_for_route_with_request(
            self.shared,
            RuntimeResponseCandidateSelection::fresh(&BTreeSet::new(), RuntimeRouteKind::Compact),
            Some(self.request_id),
            Some(spark_model),
        )?
        else {
            return Ok(false);
        };

        self.request.body =
            prodex_provider_core::provider_request_body_with_model(&self.request.body, spark_model);
        self.request_model_name = Some(spark_model.to_string());
        self.excluded_profiles.clear();
        self.last_failure = None;
        self.saw_transport_failure = false;
        self.saw_overload_failure = false;
        self.recovery_sweeps = 0;
        self.recovery_started_at = None;
        self.selection_started_at = Instant::now();
        self.selection_attempts = 0;
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} transport=http compact_model_fallback requested_model=luna effective_model={} profile={} reason=luna_capacity_unavailable_spark_available",
                self.request_id, spark_model, spark_candidate
            ),
        );
        Ok(true)
    }
}
