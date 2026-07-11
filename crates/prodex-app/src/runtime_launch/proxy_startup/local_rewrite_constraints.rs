//! Live gateway seam for the shared side-effect-free constraint planner.

use std::collections::BTreeMap;
use std::sync::Arc;

use prodex_provider_core::{
    ProviderEndpoint, ProviderRequestFeature, provider_catalog_entry,
    provider_model_fallback_chain, provider_model_from_request_body,
};

use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_credentials::RuntimeGatewayCredentialRefreshPlan;
use super::local_rewrite_gateway_route_load::RuntimeGatewayRouteLoadGuard;
use super::local_rewrite_model_memory::runtime_local_rewrite_model_scope;
use super::local_rewrite_options::RuntimeLocalRewriteProxyStartOptions;
use super::provider_bridge::{RuntimeProviderRouteKind, runtime_provider_route_kind};
use crate::{
    RuntimeProxyRequest, RuntimeRotationProxy, build_runtime_proxy_json_error_response,
    runtime_proxy_log,
};

pub(crate) fn start_runtime_local_rewrite_proxy(
    options: RuntimeLocalRewriteProxyStartOptions<'_>,
) -> anyhow::Result<RuntimeRotationProxy> {
    super::local_rewrite::start_runtime_local_rewrite_proxy_with_file_access(
        options,
        true,
        None,
        Default::default(),
    )
}

pub(crate) fn start_runtime_gateway_rewrite_proxy(
    options: RuntimeLocalRewriteProxyStartOptions<'_>,
    request_constraints: prodex_provider_core::ProviderRequestConstraintPolicy,
) -> anyhow::Result<RuntimeRotationProxy> {
    super::local_rewrite::start_runtime_local_rewrite_proxy_with_file_access(
        options,
        false,
        None,
        request_constraints,
    )
}

pub(crate) fn start_runtime_gateway_rewrite_proxy_with_secret_refresh(
    options: RuntimeLocalRewriteProxyStartOptions<'_>,
    secret_refresh: RuntimeGatewayCredentialRefreshPlan,
    request_constraints: prodex_provider_core::ProviderRequestConstraintPolicy,
) -> anyhow::Result<RuntimeRotationProxy> {
    super::local_rewrite::start_runtime_local_rewrite_proxy_with_file_access(
        options,
        false,
        Some(secret_refresh),
        request_constraints,
    )
}

pub(super) struct RuntimeGatewayPendingConstraintPlan<'a> {
    plan: Option<runtime_proxy_crate::RuntimeGatewayConstraintRoutePlan>,
    request_id: u64,
    shared: &'a RuntimeLocalRewriteProxyShared,
}

pub(super) fn runtime_gateway_prepare_constraint_plan<'a>(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &'a RuntimeLocalRewriteProxyShared,
) -> Result<Option<RuntimeGatewayPendingConstraintPlan<'a>>, tiny_http::ResponseBox> {
    let model_state = shared
        .gateway_route_load
        .lock()
        .map(|load| load.clone())
        .unwrap_or_default();
    let plan = runtime_gateway_constraint_plan(request_id, request, shared, &model_state).map_err(
        |_| {
            if let Some(endpoint) = runtime_gateway_constraint_endpoint(&request.path_and_query) {
                let trace = runtime_proxy_crate::runtime_gateway_constraint_error_trace(
                    endpoint,
                    &request.body,
                    prodex_provider_core::ProviderRequestConstraintDecision::MalformedRequestLimits,
                );
                emit_route_trace(request_id, &trace, shared);
            }
            build_runtime_proxy_json_error_response(
                422,
                "malformed_request_limits",
                "request token limits are malformed",
            )
        },
    )?;
    let Some(plan) = plan else {
        return Ok(None);
    };
    if plan.selected_model.is_none() && plan.no_route_reason.is_some() {
        emit_route_trace(request_id, &plan.trace, shared);
        return Err(build_runtime_proxy_json_error_response(
            422,
            plan.no_route_reason
                .map(|reason| reason.as_str())
                .unwrap_or("no_compatible_route"),
            "no compatible model route can satisfy this request",
        ));
    }
    Ok(Some(RuntimeGatewayPendingConstraintPlan {
        plan: Some(plan),
        request_id,
        shared,
    }))
}

impl RuntimeGatewayPendingConstraintPlan<'_> {
    pub(super) fn reject_virtual_key(
        &mut self,
        rejection: runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection,
    ) {
        let stage = match rejection {
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::MissingOrInvalidToken => {
                runtime_proxy_crate::RuntimeRouteDecisionStage::Authentication
            }
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::ModelNotAllowed => {
                runtime_proxy_crate::RuntimeRouteDecisionStage::Governance
            }
            _ => runtime_proxy_crate::RuntimeRouteDecisionStage::Admission,
        };
        self.finish_rejected(stage, rejection.code());
    }

    pub(super) fn reject_governance(&mut self, reason: &str) {
        self.finish_rejected(
            runtime_proxy_crate::RuntimeRouteDecisionStage::Governance,
            reason,
        );
    }

    pub(super) fn apply(
        &mut self,
        request: &mut RuntimeProxyRequest,
    ) -> Result<Option<RuntimeGatewayRouteLoadGuard>, tiny_http::ResponseBox> {
        let rewrite_required = self
            .plan
            .as_ref()
            .is_some_and(|plan| plan.body_rewrite_required);
        let rewritten_body = rewrite_required.then(|| {
            self.plan.as_ref().and_then(|plan| {
                runtime_proxy_crate::runtime_gateway_apply_constraint_plan_body(&request.body, plan)
            })
        });
        if matches!(rewritten_body, Some(None)) {
            self.finish_rejected(
                runtime_proxy_crate::RuntimeRouteDecisionStage::ModelResolution,
                "route_rewrite_failed",
            );
            return Err(build_runtime_proxy_json_error_response(
                422,
                "route_rewrite_failed",
                "selected model route could not be applied safely",
            ));
        }
        let Some(plan) = self.plan.take() else {
            return Ok(None);
        };
        let model = plan.selected_model.as_deref();
        let alias = self
            .shared
            .gateway_route_aliases
            .iter()
            .find(|alias| alias.alias == plan.requested_model);
        let guard = alias.zip(model).map(|(_, model)| {
            RuntimeGatewayRouteLoadGuard::enter(
                Arc::clone(&self.shared.gateway_route_load),
                model,
                &request.body,
            )
        });
        if let Some((alias, model)) = alias.zip(model) {
            runtime_proxy_log(
                &self.shared.runtime_shared,
                runtime_proxy_crate::runtime_proxy_structured_log_message(
                    "gateway_route_alias_rewrite",
                    [
                        runtime_proxy_crate::runtime_proxy_log_field(
                            "request",
                            self.request_id.to_string(),
                        ),
                        runtime_proxy_crate::runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_crate::runtime_proxy_log_field("alias", alias.alias.as_str()),
                        runtime_proxy_crate::runtime_proxy_log_field(
                            "strategy",
                            alias.strategy.as_str(),
                        ),
                        runtime_proxy_crate::runtime_proxy_log_field("model", model),
                        runtime_proxy_crate::runtime_proxy_log_field(
                            "path",
                            runtime_proxy_crate::path_without_query(&request.path_and_query),
                        ),
                    ],
                ),
            );
        }
        if let Some(adjustment) = plan.adjustment.as_ref() {
            runtime_proxy_log(
                &self.shared.runtime_shared,
                runtime_proxy_crate::runtime_proxy_structured_log_message(
                    "gateway_output_limit_adjusted",
                    [
                        runtime_proxy_crate::runtime_proxy_log_field(
                            "request",
                            self.request_id.to_string(),
                        ),
                        runtime_proxy_crate::runtime_proxy_log_field(
                            "reason",
                            adjustment.reason.as_str(),
                        ),
                        runtime_proxy_crate::runtime_proxy_log_field(
                            "requested_tokens",
                            adjustment.requested_tokens.to_string(),
                        ),
                        runtime_proxy_crate::runtime_proxy_log_field(
                            "applied_tokens",
                            adjustment.applied_tokens.to_string(),
                        ),
                    ],
                ),
            );
        }
        if let Some(Some(rewritten_body)) = rewritten_body {
            request.body = rewritten_body;
        }
        let mut trace = plan.trace;
        for stage in [
            runtime_proxy_crate::RuntimeRouteDecisionStage::Governance,
            runtime_proxy_crate::RuntimeRouteDecisionStage::Authentication,
            runtime_proxy_crate::RuntimeRouteDecisionStage::Admission,
        ] {
            record_trace_stage(
                &mut trace,
                stage,
                runtime_proxy_crate::RuntimeRouteDecisionStageOutcome::Passed,
            );
        }
        emit_route_trace(self.request_id, &trace, self.shared);
        Ok(guard)
    }

    fn finish_rejected(
        &mut self,
        stage: runtime_proxy_crate::RuntimeRouteDecisionStage,
        reason: &str,
    ) {
        let Some(plan) = self.plan.take() else {
            return;
        };
        let mut trace = plan.trace;
        record_trace_stage(
            &mut trace,
            stage,
            runtime_proxy_crate::RuntimeRouteDecisionStageOutcome::Rejected,
        );
        trace.terminal_outcome = runtime_proxy_crate::RuntimeRouteDecisionTerminalOutcome::Failed;
        trace.terminal_reason = Some(runtime_proxy_crate::RuntimeRouteDecisionReason::from_label(
            reason,
        ));
        emit_route_trace(self.request_id, &trace, self.shared);
    }
}

impl Drop for RuntimeGatewayPendingConstraintPlan<'_> {
    fn drop(&mut self) {
        if self.plan.is_some() {
            self.finish_rejected(
                runtime_proxy_crate::RuntimeRouteDecisionStage::Admission,
                "request_aborted_precommit",
            );
        }
    }
}

fn record_trace_stage(
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTrace,
    stage: runtime_proxy_crate::RuntimeRouteDecisionStage,
    outcome: runtime_proxy_crate::RuntimeRouteDecisionStageOutcome,
) {
    if let Some(summary) = trace
        .stages
        .iter_mut()
        .find(|summary| summary.stage == stage)
    {
        summary.outcome = outcome;
    } else if trace.stages.len() < runtime_proxy_crate::RUNTIME_ROUTE_DECISION_TRACE_MAX_STAGES {
        trace
            .stages
            .push(runtime_proxy_crate::RuntimeRouteDecisionStageSummary { stage, outcome });
    } else {
        trace.truncation.truncated = true;
        trace.truncation.omitted_stages = trace.truncation.omitted_stages.saturating_add(1);
    }
    trace.stages.sort_by_key(|summary| summary.stage);
}

fn emit_route_trace(
    request_id: u64,
    trace: &runtime_proxy_crate::RuntimeRouteDecisionTrace,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    let mut fields = vec![
        runtime_proxy_crate::runtime_proxy_log_field("request", request_id.to_string()),
        runtime_proxy_crate::runtime_proxy_log_field("transport", "http"),
        runtime_proxy_crate::runtime_proxy_log_field(
            "trace",
            serde_json::to_string(trace).unwrap_or_else(|_| "{}".to_string()),
        ),
    ];
    if let Some(call_id) = shared
        .gateway_usage
        .call_ids
        .lock()
        .ok()
        .and_then(|call_ids| call_ids.get(&request_id).cloned())
    {
        fields.push(runtime_proxy_crate::runtime_proxy_log_field(
            "call_id", call_id,
        ));
    }
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_crate::runtime_proxy_structured_log_message("route_decision", fields),
    );
}

pub(super) fn runtime_gateway_constraint_plan(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    model_state: &BTreeMap<String, runtime_proxy_crate::RuntimeGatewayRouteModelState>,
) -> Result<
    Option<runtime_proxy_crate::RuntimeGatewayConstraintRoutePlan>,
    prodex_provider_core::ProviderRequestLimitError,
> {
    let policy = shared.gateway_request_constraints;
    let Some(endpoint) = runtime_gateway_constraint_endpoint(&request.path_and_query) else {
        return Ok(None);
    };
    let provider = shared.provider.bridge_kind().provider_id();
    let (owner_model, continuation_required) = if policy.enabled {
        let remembered_owner = runtime_local_rewrite_model_scope(
            shared.provider.bridge_kind(),
            request,
            &request.body,
        )
        .and_then(|scope| shared.model_memory.lock().ok()?.selected_model(&scope));
        let continuation_required =
            runtime_proxy_crate::runtime_request_previous_response_id(request).is_some()
                || request.headers.iter().any(|(name, value)| {
                    name.eq_ignore_ascii_case("x-codex-turn-state") && !value.trim().is_empty()
                });
        let owner_model = remembered_owner.or_else(|| {
            continuation_required
                .then(|| exact_request_model(provider, request, &shared.gateway_route_aliases))
                .flatten()
        });
        (owner_model, continuation_required)
    } else {
        (None, false)
    };
    runtime_proxy_crate::runtime_gateway_plan_route_with_constraints(
        provider,
        endpoint,
        &request.body,
        runtime_proxy_crate::RuntimeGatewayConstraintPlanInput {
            aliases: &shared.gateway_route_aliases,
            diagnostic_seed: request_id,
            model_state,
            policy,
            additional_features: &[] as &[ProviderRequestFeature],
            configured_reasoning_reserve_tokens: shared
                .provider
                .configured_reasoning_reserve_tokens(),
            hard_affinity_model: owner_model.as_deref(),
            hard_affinity_required: continuation_required,
        },
    )
    .map(Some)
}

fn exact_request_model(
    provider: prodex_provider_core::ProviderId,
    request: &RuntimeProxyRequest,
    aliases: &[runtime_proxy_crate::RuntimeGatewayRouteAlias],
) -> Option<String> {
    let model = provider_model_from_request_body(&request.body)?;
    if aliases.iter().any(|alias| alias.alias == model) {
        return None;
    }
    let chain = provider_model_fallback_chain(provider, &model);
    if chain.len() != 1 || !chain[0].eq_ignore_ascii_case(&model) {
        return None;
    }
    if let Some(entry) = provider_catalog_entry(provider, &model)
        && !entry.id.eq_ignore_ascii_case(&model)
    {
        return None;
    }
    Some(model)
}

fn runtime_gateway_constraint_endpoint(path_and_query: &str) -> Option<ProviderEndpoint> {
    match runtime_provider_route_kind(runtime_proxy_crate::path_without_query(path_and_query))? {
        RuntimeProviderRouteKind::Responses => Some(ProviderEndpoint::Responses),
        RuntimeProviderRouteKind::ResponsesCompact => Some(ProviderEndpoint::ResponsesCompact),
        RuntimeProviderRouteKind::ChatCompletions => Some(ProviderEndpoint::ChatCompletions),
        RuntimeProviderRouteKind::Messages => Some(ProviderEndpoint::Messages),
        RuntimeProviderRouteKind::Embeddings => Some(ProviderEndpoint::Embeddings),
        RuntimeProviderRouteKind::ModelsList | RuntimeProviderRouteKind::ModelsSingle(_) => None,
    }
}
