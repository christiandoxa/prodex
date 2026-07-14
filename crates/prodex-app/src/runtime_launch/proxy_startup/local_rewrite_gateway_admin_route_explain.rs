use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_admin_audit::{
    RuntimeGatewayAdminRouteExplainAudit, runtime_gateway_audit_admin_request_denied_event,
    runtime_gateway_audit_admin_role_denied_event, runtime_gateway_audit_admin_route_explain_event,
};
use super::local_rewrite_gateway_admin_auth::{
    RuntimeGatewayAdminAuth, runtime_gateway_admin_auth_is_unscoped,
};
use super::local_rewrite_gateway_admin_response::{
    RuntimeGatewayAdminError, runtime_gateway_admin_json_response,
};
use super::local_rewrite_gateway_admin_router::{
    runtime_gateway_admin_route_explain_plan, runtime_gateway_http_request_meta,
};
use super::{RuntimeProxyRequest, build_runtime_proxy_json_error_response, path_without_query};
use prodex_provider_core::{
    ProviderEndpoint, ProviderRequestConstraintDecision, ProviderRequestConstraintPolicy,
    ProviderRequestFeature,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

pub(super) const RUNTIME_GATEWAY_ROUTE_EXPLAIN_MAX_BODY_BYTES: usize = 256 * 1024;
const RUNTIME_GATEWAY_ROUTE_EXPLAIN_MAX_FEATURES: usize = 16;
const RUNTIME_GATEWAY_ROUTE_EXPLAIN_MAX_RESPONSE_ITEMS: usize =
    runtime_proxy_crate::RUNTIME_ROUTE_DECISION_TRACE_MAX_CANDIDATES;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeGatewayRouteExplainRequest {
    endpoint: ProviderEndpoint,
    requested_model: String,
    #[serde(default = "runtime_gateway_route_explain_empty_request")]
    request: serde_json::Value,
    #[serde(default)]
    required_capabilities: Vec<ProviderRequestFeature>,
    #[serde(default)]
    include_current_state: bool,
    #[serde(default = "runtime_gateway_route_explain_default_seed")]
    diagnostic_seed: u64,
    #[serde(default)]
    hard_affinity_required: bool,
    #[serde(default)]
    owner_model: Option<String>,
    #[serde(default)]
    policy: Option<ProviderRequestConstraintPolicy>,
}

#[derive(Debug)]
struct RuntimeGatewayRouteExplainInput {
    endpoint: ProviderEndpoint,
    body: Vec<u8>,
    required_capabilities: Vec<ProviderRequestFeature>,
    include_current_state: bool,
    diagnostic_seed: u64,
    hard_affinity_required: bool,
    owner_model: Option<String>,
    policy: Option<ProviderRequestConstraintPolicy>,
}

pub(super) fn runtime_gateway_admin_route_explain_response(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let path = path_without_query(&captured.path_and_query);
    let http = runtime_gateway_http_request_meta(captured, path);
    let Some(control_plane_plan) = runtime_gateway_admin_route_explain_plan(&http, admin_auth)
    else {
        runtime_gateway_audit_admin_role_denied_event(shared, admin_auth, &captured.method, path);
        return build_runtime_proxy_json_error_response(
            403,
            "gateway_admin_role_forbidden",
            "gateway admin role does not allow route explanation",
        );
    };

    let input = match runtime_gateway_route_explain_input(captured) {
        Ok(input) => input,
        Err(error) => {
            runtime_gateway_audit_admin_request_denied_event(
                shared,
                admin_auth,
                error.code(),
                &captured.method,
                path,
            );
            return error.into_response();
        }
    };
    if input.include_current_state && !runtime_gateway_admin_auth_is_unscoped(admin_auth) {
        let error = RuntimeGatewayAdminError::new(
            403,
            "gateway_route_state_scope_forbidden",
            "scoped gateway principals cannot inspect global route state",
        );
        runtime_gateway_audit_admin_request_denied_event(
            shared,
            admin_auth,
            error.code(),
            &captured.method,
            path,
        );
        return error.into_response();
    }
    let (model_state, current_load_truncated) = if input.include_current_state {
        match shared.gateway_route_load.lock() {
            Ok(state) => runtime_gateway_route_explain_model_state_snapshot(&state),
            Err(_) => {
                let error = RuntimeGatewayAdminError::new(
                    503,
                    "gateway_route_state_unavailable",
                    "current gateway route state is unavailable",
                );
                runtime_gateway_audit_admin_request_denied_event(
                    shared,
                    admin_auth,
                    error.code(),
                    &captured.method,
                    path,
                );
                return error.into_response();
            }
        }
    } else {
        (BTreeMap::new(), false)
    };
    let plan = match runtime_proxy_crate::runtime_gateway_plan_route_with_constraints(
        shared.provider.bridge_kind().provider_id(),
        input.endpoint,
        &input.body,
        runtime_proxy_crate::RuntimeGatewayConstraintPlanInput {
            aliases: &shared.gateway_route_aliases,
            diagnostic_seed: input.diagnostic_seed,
            model_state: &model_state,
            policy: input.policy.unwrap_or(shared.gateway_request_constraints),
            additional_features: &input.required_capabilities,
            configured_reasoning_reserve_tokens: shared
                .provider
                .configured_reasoning_reserve_tokens(),
            hard_affinity_model: input.owner_model.as_deref(),
            hard_affinity_required: input.hard_affinity_required,
        },
    ) {
        Ok(plan) => plan,
        Err(_) => {
            let error = RuntimeGatewayAdminError::new(
                422,
                "invalid_request_limits",
                "request token limits are malformed",
            );
            runtime_gateway_audit_admin_request_denied_event(
                shared,
                admin_auth,
                error.code(),
                &captured.method,
                path,
            );
            return error.into_response();
        }
    };

    let rendered = runtime_gateway_route_explain_response_body(
        &plan,
        input.include_current_state,
        current_load_truncated,
        input.diagnostic_seed,
        input.hard_affinity_required,
        input.owner_model.as_deref(),
    );
    let result_category = if rendered.selected_candidate.is_some() {
        "selected"
    } else {
        "no_route"
    };
    let control_plane_action = control_plane_plan.audit_event.action.as_str();
    runtime_gateway_audit_admin_route_explain_event(
        shared,
        admin_auth,
        RuntimeGatewayAdminRouteExplainAudit {
            control_plane_action,
            endpoint: input.endpoint.label(),
            requested_model: &rendered.requested_model,
            result_category,
            selected_route_id: rendered.selected_candidate.as_deref(),
            candidate_count: plan.concrete_candidates.len(),
            diagnostic_seed: input.diagnostic_seed,
            current_load_included: input.include_current_state,
            health_quota_included: false,
            hard_affinity_required: input.hard_affinity_required,
            hard_affinity_applied: input.hard_affinity_required || input.owner_model.is_some(),
            truncated: rendered.truncated,
        },
    );

    runtime_gateway_admin_json_response(200, rendered.body)
}

struct RuntimeGatewayRouteExplainRendered {
    body: serde_json::Value,
    requested_model: String,
    selected_candidate: Option<String>,
    truncated: bool,
}

fn runtime_gateway_route_explain_response_body(
    plan: &runtime_proxy_crate::RuntimeGatewayConstraintRoutePlan,
    include_current_state: bool,
    current_load_truncated: bool,
    diagnostic_seed: u64,
    hard_affinity_required: bool,
    owner_model: Option<&str>,
) -> RuntimeGatewayRouteExplainRendered {
    let mut truncated = plan.trace.truncation.truncated || plan.truncated || current_load_truncated;
    let alias_resolution_chain =
        runtime_gateway_route_explain_safe_identifiers(&plan.alias_chain, &mut truncated);
    let candidate_models = plan
        .concrete_candidates
        .iter()
        .map(|candidate| candidate.model.clone())
        .collect::<Vec<_>>();
    let concrete_candidate_models =
        runtime_gateway_route_explain_safe_identifiers(&candidate_models, &mut truncated);
    let mut requirements = plan.requirements.clone();
    requirements.requested_model = runtime_gateway_route_explain_safe_identifier(
        &requirements.requested_model,
        &mut truncated,
    );
    requirements.resolved_upstream_model = requirements
        .resolved_upstream_model
        .as_deref()
        .map(|model| runtime_gateway_route_explain_safe_identifier(model, &mut truncated));
    let mut warnings = runtime_gateway_route_explain_warnings(&plan.concrete_candidates);
    if include_current_state {
        warnings.push("health_quota_state_not_available");
    }
    if current_load_truncated {
        warnings.push("current_load_snapshot_truncated");
    }
    let requested_model = plan
        .trace
        .requested_model
        .clone()
        .unwrap_or_else(|| "redacted".to_string());
    let selected_candidate = plan.trace.selected_candidate.clone();
    let owner_model = owner_model
        .map(|model| runtime_gateway_route_explain_safe_identifier(model, &mut truncated));
    let hard_affinity_applied = hard_affinity_required || owner_model.is_some();
    let body = serde_json::json!({
        "object": "gateway.route_explanation",
        "schema_version": plan.trace.schema_version,
        "diagnostic_seed": diagnostic_seed,
        "hard_affinity_required": hard_affinity_required,
        "hard_affinity_applied": hard_affinity_applied,
        "owner_model": owner_model,
        "requested_model": requested_model,
        "resolved_model": plan.trace.resolved_model.clone(),
        "alias_resolution_chain": alias_resolution_chain,
        "concrete_candidate_models": concrete_candidate_models,
        "request_requirements": requirements,
        "candidate_matrix": plan.trace.candidates.clone(),
        "selected_candidate": selected_candidate,
        "policy_adjustments": plan.adjustment.iter().cloned().collect::<Vec<_>>(),
        "current_load_included": include_current_state,
        "health_quota_included": false,
        "warnings": warnings,
        "final_no_route_reason": plan.no_route_reason.map(|reason| reason.as_str()),
        "truncated": truncated,
        "omitted_candidates": plan.omitted_candidates,
        "trace": plan.trace.clone(),
    });
    RuntimeGatewayRouteExplainRendered {
        body,
        requested_model,
        selected_candidate,
        truncated,
    }
}

fn runtime_gateway_route_explain_model_state_snapshot(
    state: &BTreeMap<String, runtime_proxy_crate::RuntimeGatewayRouteModelState>,
) -> (
    BTreeMap<String, runtime_proxy_crate::RuntimeGatewayRouteModelState>,
    bool,
) {
    let limit = runtime_proxy_crate::RUNTIME_GATEWAY_CONSTRAINT_PLANNER_MAX_CANDIDATES;
    (
        state
            .iter()
            .take(limit)
            .map(|(model, state)| (model.clone(), state.clone()))
            .collect(),
        state.len() > limit,
    )
}

fn runtime_gateway_route_explain_input(
    captured: &RuntimeProxyRequest,
) -> Result<RuntimeGatewayRouteExplainInput, RuntimeGatewayAdminError> {
    if captured.body.len() > RUNTIME_GATEWAY_ROUTE_EXPLAIN_MAX_BODY_BYTES {
        return Err(RuntimeGatewayAdminError::new(
            413,
            "request_body_too_large",
            "route explanation request body is too large",
        ));
    }
    let value = serde_json::from_slice::<serde_json::Value>(&captured.body).map_err(|_| {
        RuntimeGatewayAdminError::new(400, "invalid_json", "request body is not valid JSON")
    })?;
    let mut request =
        serde_json::from_value::<RuntimeGatewayRouteExplainRequest>(value).map_err(|_| {
            RuntimeGatewayAdminError::new(
                422,
                "invalid_route_explain_request",
                "route explanation request fields are invalid",
            )
        })?;
    if !matches!(
        request.endpoint,
        ProviderEndpoint::Responses
            | ProviderEndpoint::ResponsesCompact
            | ProviderEndpoint::ChatCompletions
            | ProviderEndpoint::Messages
            | ProviderEndpoint::Embeddings
    ) {
        return Err(RuntimeGatewayAdminError::new(
            422,
            "unsupported_route_endpoint",
            "route explanation endpoint is unsupported",
        ));
    }
    if !runtime_gateway_route_explain_identifier_is_valid(&request.requested_model) {
        return Err(RuntimeGatewayAdminError::new(
            422,
            "invalid_requested_model",
            "requested_model must be an exact non-empty model identifier",
        ));
    }
    if request
        .owner_model
        .as_deref()
        .is_some_and(|model| !runtime_gateway_route_explain_identifier_is_valid(model))
    {
        return Err(RuntimeGatewayAdminError::new(
            422,
            "invalid_owner_model",
            "owner_model must be an exact non-empty model identifier",
        ));
    }
    if request.required_capabilities.len() > RUNTIME_GATEWAY_ROUTE_EXPLAIN_MAX_FEATURES {
        return Err(RuntimeGatewayAdminError::new(
            422,
            "too_many_required_capabilities",
            "too many required route capabilities were supplied",
        ));
    }
    let Some(object) = request.request.as_object_mut() else {
        return Err(RuntimeGatewayAdminError::new(
            422,
            "invalid_route_request",
            "request must be a JSON object",
        ));
    };
    object.insert(
        "model".to_string(),
        serde_json::Value::String(request.requested_model.clone()),
    );
    if request
        .policy
        .is_some_and(|policy| policy.safe_window_tokens == 0)
    {
        return Err(RuntimeGatewayAdminError::new(
            422,
            "invalid_safe_window_tokens",
            "safe_window_tokens must be greater than zero",
        ));
    }
    let body = serde_json::to_vec(&request.request).map_err(|_| {
        RuntimeGatewayAdminError::new(
            422,
            "invalid_route_request",
            "request could not be normalized",
        )
    })?;
    Ok(RuntimeGatewayRouteExplainInput {
        endpoint: request.endpoint,
        body,
        required_capabilities: request.required_capabilities,
        include_current_state: request.include_current_state,
        diagnostic_seed: request.diagnostic_seed,
        hard_affinity_required: request.hard_affinity_required,
        owner_model: request.owner_model,
        policy: request.policy,
    })
}

fn runtime_gateway_route_explain_identifier_is_valid(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_whitespace)
}

fn runtime_gateway_route_explain_empty_request() -> serde_json::Value {
    serde_json::json!({})
}

fn runtime_gateway_route_explain_default_seed() -> u64 {
    1
}

fn runtime_gateway_route_explain_safe_identifiers(
    values: &[String],
    truncated: &mut bool,
) -> Vec<String> {
    if values.len() > RUNTIME_GATEWAY_ROUTE_EXPLAIN_MAX_RESPONSE_ITEMS {
        *truncated = true;
    }
    values
        .iter()
        .take(RUNTIME_GATEWAY_ROUTE_EXPLAIN_MAX_RESPONSE_ITEMS)
        .map(|value| runtime_gateway_route_explain_safe_identifier(value, truncated))
        .collect()
}

fn runtime_gateway_route_explain_safe_identifier(value: &str, truncated: &mut bool) -> String {
    let (value, was_truncated) = runtime_proxy_crate::runtime_route_decision_safe_identifier(value);
    *truncated |= was_truncated;
    value
}

fn runtime_gateway_route_explain_warnings(
    candidates: &[runtime_proxy_crate::RuntimeGatewayConstraintCandidate],
) -> Vec<&'static str> {
    let mut warnings = BTreeSet::new();
    for candidate in candidates {
        for warning in &candidate.evaluation.warnings {
            warnings.insert(warning.as_str());
        }
        if candidate.evaluation.decision
            == ProviderRequestConstraintDecision::CatalogEntryUnavailable
        {
            warnings.insert(ProviderRequestConstraintDecision::CatalogEntryUnavailable.as_str());
        }
    }
    warnings.into_iter().take(32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_output_plan(
        decision: prodex_provider_core::ProviderRequestConstraintDecision,
        eligible: bool,
        adjustment: Option<prodex_provider_core::ProviderOutputAdjustment>,
    ) -> runtime_proxy_crate::RuntimeGatewayConstraintRoutePlan {
        let requirements = prodex_provider_core::ProviderRequestRequirements {
            endpoint: ProviderEndpoint::Responses,
            requested_model: "gpt-test".to_string(),
            resolved_upstream_model: Some("gpt-test".to_string()),
            estimated_input_tokens: 10,
            explicit_output_tokens: Some(200),
            output_limit_field: Some(
                prodex_provider_core::ProviderOutputLimitField::MaxOutputTokens,
            ),
            default_output_reserve_tokens: None,
            reasoning_effort: None,
            reasoning_reserve_tokens: None,
            total_required_tokens: 210,
            required_features: Vec::new(),
        };
        let evaluation = prodex_provider_core::ProviderRequestConstraintEvaluation {
            decision,
            eligible,
            requirements: requirements.clone(),
            missing_feature: None,
            available_context_tokens: Some(128_000),
            max_output_tokens: Some(100),
            adjustment: adjustment.clone(),
            warnings: adjustment
                .as_ref()
                .map(|_| vec![decision])
                .unwrap_or_default(),
        };
        let candidate = runtime_proxy_crate::RuntimeGatewayConstraintCandidate {
            model: "gpt-test".to_string(),
            original_order: 0,
            selected: eligible,
            evaluation,
        };
        let mut trace = runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder::new(
            runtime_proxy_crate::RuntimeRouteDecisionRoute::Responses,
            Some("gpt-test"),
        );
        trace.record_stage(
            runtime_proxy_crate::RuntimeRouteDecisionStage::RequestConstraints,
            if eligible {
                runtime_proxy_crate::RuntimeRouteDecisionStageOutcome::Passed
            } else {
                runtime_proxy_crate::RuntimeRouteDecisionStageOutcome::Rejected
            },
        );
        let mut trace_candidate = runtime_proxy_crate::RuntimeRouteCandidateDecisionInput::eligible(
            0,
            runtime_proxy_crate::RuntimeRouteCandidateClass::Fallback,
        );
        trace_candidate.provider = Some("openai".to_string());
        trace_candidate.model = Some("gpt-test".to_string());
        trace_candidate.eligibility = if eligible {
            runtime_proxy_crate::RuntimeRouteCandidateEligibility::Eligible
        } else {
            runtime_proxy_crate::RuntimeRouteCandidateEligibility::Rejected
        };
        trace_candidate.rejection_stage = (!eligible)
            .then_some(runtime_proxy_crate::RuntimeRouteDecisionStage::RequestConstraints);
        trace_candidate.reason = Some(runtime_proxy_crate::RuntimeRouteDecisionReason::from_label(
            decision.as_str(),
        ));
        trace_candidate.selected = eligible;
        trace_candidate.diagnostics = runtime_proxy_crate::RuntimeRouteDecisionDiagnostics {
            estimated_input_tokens: Some(10),
            output_tokens: Some(200),
            total_required_tokens: Some(210),
            available_context_tokens: Some(128_000),
            max_output_tokens: Some(100),
            requested_output_tokens: adjustment.as_ref().map(|value| value.requested_tokens),
            applied_output_tokens: adjustment.as_ref().map(|value| value.applied_tokens),
            ..runtime_proxy_crate::RuntimeRouteDecisionDiagnostics::default()
        };
        trace.record_candidate("gpt-test", trace_candidate);
        if eligible {
            trace.mark_selected("gpt-test");
            trace.set_resolved_model(Some("gpt-test"));
        }
        let trace = trace.finish(
            if eligible {
                runtime_proxy_crate::RuntimeRouteDecisionTerminalOutcome::Selected
            } else {
                runtime_proxy_crate::RuntimeRouteDecisionTerminalOutcome::NoCandidate
            },
            (!eligible).then(|| {
                runtime_proxy_crate::RuntimeRouteDecisionReason::from_label(decision.as_str())
            }),
        );
        runtime_proxy_crate::RuntimeGatewayConstraintRoutePlan {
            requested_model: "gpt-test".to_string(),
            alias_chain: vec!["gpt-test".to_string()],
            concrete_candidates: vec![candidate],
            requirements,
            selected_model: eligible.then(|| "gpt-test".to_string()),
            upstream_attempt_model: None,
            body_rewrite_required: false,
            adjustment,
            no_route_reason: (!eligible).then_some(decision),
            trace,
            truncated: false,
            selection_pool_truncated: false,
            omitted_candidates: 0,
        }
    }

    fn request(body: serde_json::Value) -> RuntimeProxyRequest {
        RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/v1/prodex/gateway/routes/explain".to_string(),
            headers: Vec::new(),
            body: serde_json::to_vec(&body).unwrap(),
        }
    }

    #[test]
    fn route_explain_input_rejects_unknown_fields_invalid_windows_and_non_object_requests() {
        let unknown = request(serde_json::json!({
            "endpoint": "responses",
            "requested_model": "gpt-test",
            "required_capabilities": ["credential_dump"]
        }));
        assert_eq!(
            runtime_gateway_route_explain_input(&unknown)
                .unwrap_err()
                .test_code(),
            "invalid_route_explain_request"
        );

        let unknown_policy = request(serde_json::json!({
            "endpoint": "responses",
            "requested_model": "gpt-test",
            "policy": {"unbounded_override": true}
        }));
        assert_eq!(
            runtime_gateway_route_explain_input(&unknown_policy)
                .unwrap_err()
                .test_code(),
            "invalid_route_explain_request"
        );

        let zero_window = request(serde_json::json!({
            "endpoint": "responses",
            "requested_model": "gpt-test",
            "policy": {"unknown_context": "allow", "safe_window_tokens": 0}
        }));
        assert_eq!(
            runtime_gateway_route_explain_input(&zero_window)
                .unwrap_err()
                .test_code(),
            "invalid_safe_window_tokens"
        );

        let non_object = request(serde_json::json!({
            "endpoint": "responses",
            "requested_model": "gpt-test",
            "request": "prompt text"
        }));
        assert_eq!(
            runtime_gateway_route_explain_input(&non_object)
                .unwrap_err()
                .test_code(),
            "invalid_route_request"
        );
    }

    #[test]
    fn route_explain_input_overwrites_nested_model_without_echoing_payload() {
        let captured = request(serde_json::json!({
            "endpoint": "responses",
            "requested_model": "gpt-test",
            "request": {"model": "wrong", "input": "sensitive prompt"}
        }));
        let input = runtime_gateway_route_explain_input(&captured).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&input.body).unwrap();

        assert_eq!(value["model"], "gpt-test");
    }

    #[test]
    fn route_explain_response_surfaces_output_limit_rejection() {
        let plan = synthetic_output_plan(
            ProviderRequestConstraintDecision::RequestedOutputExceedsModelLimit,
            false,
            None,
        );
        let response =
            runtime_gateway_route_explain_response_body(&plan, false, false, 11, false, None).body;

        assert_eq!(response["diagnostic_seed"], 11);
        assert_eq!(response["current_load_included"], false);
        assert_eq!(response["candidate_matrix"][0]["eligibility"], "rejected");
        assert_eq!(
            response["candidate_matrix"][0]["reason"],
            "requested_output_exceeds_model_limit"
        );
        assert_eq!(
            response["final_no_route_reason"],
            "requested_output_exceeds_model_limit"
        );
        assert!(response["selected_candidate"].is_null());
    }

    #[test]
    fn route_explain_response_surfaces_output_limit_clamp_adjustment() {
        let adjustment = prodex_provider_core::ProviderOutputAdjustment {
            field: prodex_provider_core::ProviderOutputLimitField::MaxOutputTokens,
            requested_tokens: 200,
            applied_tokens: 100,
            reason: ProviderRequestConstraintDecision::OutputLimitClamped,
        };
        let plan = synthetic_output_plan(
            ProviderRequestConstraintDecision::OutputLimitClamped,
            true,
            Some(adjustment),
        );
        let response = runtime_gateway_route_explain_response_body(
            &plan,
            true,
            false,
            17,
            true,
            Some("gpt-owner"),
        )
        .body;

        assert_eq!(response["diagnostic_seed"], 17);
        assert_eq!(response["current_load_included"], true);
        assert_eq!(response["health_quota_included"], false);
        assert_eq!(response["hard_affinity_required"], true);
        assert_eq!(response["hard_affinity_applied"], true);
        assert_eq!(response["owner_model"], "gpt-owner");
        assert_eq!(response["policy_adjustments"][0]["requested_tokens"], 200);
        assert_eq!(response["policy_adjustments"][0]["applied_tokens"], 100);
        assert_eq!(
            response["policy_adjustments"][0]["reason"],
            "output_limit_clamped"
        );
        assert_eq!(
            response["candidate_matrix"][0]["diagnostics"]["applied_output_tokens"],
            100
        );
        assert!(
            response["warnings"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("output_limit_clamped"))
        );
        assert_eq!(response["omitted_candidates"], 0);
    }

    #[test]
    fn route_explain_current_load_snapshot_is_bounded_and_reported() {
        let limit = runtime_proxy_crate::RUNTIME_GATEWAY_CONSTRAINT_PLANNER_MAX_CANDIDATES;
        let state = (0..=limit)
            .map(|index| {
                (
                    format!("model-{index:04}"),
                    runtime_proxy_crate::RuntimeGatewayRouteModelState::default(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let (snapshot, truncated) = runtime_gateway_route_explain_model_state_snapshot(&state);
        assert_eq!(snapshot.len(), limit);
        assert!(truncated);

        let plan = synthetic_output_plan(ProviderRequestConstraintDecision::Compatible, true, None);
        let response =
            runtime_gateway_route_explain_response_body(&plan, true, true, 1, false, None).body;
        assert_eq!(response["truncated"], true);
        assert!(
            response["warnings"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("current_load_snapshot_truncated"))
        );
    }
}
