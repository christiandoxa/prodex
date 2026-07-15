use super::{
    RuntimeProviderBridgeKind, RuntimeProviderRouteKind, runtime_provider_label,
    runtime_provider_model_from_body, runtime_provider_route_kind,
};
use prodex_provider_core::{
    HarnessProviderPolicySpec, ProviderEndpoint, ProviderId, ProviderTransformInput,
    ProviderTransformLoss, ProviderTransformResult, provider_translator,
};
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use std::collections::BTreeMap;

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_request_conformance_result(
    kind: RuntimeProviderBridgeKind,
    request: &crate::RuntimeProxyRequest,
    body: &[u8],
) -> Option<ProviderTransformResult> {
    let path = path_without_query(&request.path_and_query);
    let endpoint = match runtime_provider_route_kind(path) {
        Some(RuntimeProviderRouteKind::Responses) => ProviderEndpoint::Responses,
        Some(RuntimeProviderRouteKind::ResponsesCompact) => ProviderEndpoint::ResponsesCompact,
        _ => return None,
    };
    let mut input = ProviderTransformInput::new(endpoint, body.to_vec());
    input.model = runtime_provider_model_from_body(body);
    input.headers = runtime_provider_conformance_headers(&request.headers);
    Some(provider_translator(kind.provider_id()).transform_request(input))
}

fn runtime_provider_conformance_headers(headers: &[(String, String)]) -> BTreeMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            let name = name.to_ascii_lowercase();
            matches!(name.as_str(), "x-codex-turn-state" | "session_id")
                .then(|| (name, value.clone()))
        })
        .collect()
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_log_request_conformance(
    shared: &crate::RuntimeRotationProxyShared,
    request_id: u64,
    kind: RuntimeProviderBridgeKind,
    result: &ProviderTransformResult,
) {
    runtime_provider_log_conformance(
        shared,
        request_id,
        kind,
        result,
        "local_rewrite_provider_conformance_request",
    );
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_response_conformance_result(
    kind: RuntimeProviderBridgeKind,
    status: u16,
    body: &[u8],
) -> Option<ProviderTransformResult> {
    if !(200..300).contains(&status) {
        return None;
    }
    let mut input = ProviderTransformInput::new(ProviderEndpoint::Responses, body.to_vec());
    input.status = Some(status);
    Some(provider_translator(kind.provider_id()).transform_response(input))
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_log_response_conformance(
    shared: &crate::RuntimeRotationProxyShared,
    request_id: u64,
    kind: RuntimeProviderBridgeKind,
    result: &ProviderTransformResult,
) {
    runtime_provider_log_conformance(
        shared,
        request_id,
        kind,
        result,
        "local_rewrite_provider_conformance_response",
    );
}

pub(in crate::runtime_launch::proxy_startup) struct RuntimeHarnessProviderPolicyLog<'a> {
    pub(in crate::runtime_launch::proxy_startup) provider: ProviderId,
    pub(in crate::runtime_launch::proxy_startup) endpoint: ProviderEndpoint,
    pub(in crate::runtime_launch::proxy_startup) model: &'a str,
    pub(in crate::runtime_launch::proxy_startup) phase: &'static str,
    pub(in crate::runtime_launch::proxy_startup) policy: Option<&'static HarnessProviderPolicySpec>,
    pub(in crate::runtime_launch::proxy_startup) applied: bool,
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_harness_log_provider_policy(
    shared: &crate::RuntimeRotationProxyShared,
    request_id: u64,
    event: RuntimeHarnessProviderPolicyLog<'_>,
) {
    let RuntimeHarnessProviderPolicyLog {
        provider,
        endpoint,
        model,
        phase,
        policy,
        applied,
    } = event;
    let Some(policy) = policy else {
        return;
    };
    crate::runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "harness_provider_policy",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("provider", provider.label()),
                runtime_proxy_log_field("route", endpoint.label()),
                runtime_proxy_log_field("model", model),
                runtime_proxy_log_field("phase", phase),
                runtime_proxy_log_field("evaluation_id", policy.evaluation_id),
                runtime_proxy_log_field(
                    "evaluation_version",
                    policy.evaluation_version.to_string(),
                ),
                runtime_proxy_log_field("applied", applied.to_string()),
            ],
        ),
    );
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_stream_event_conformance_result(
    kind: RuntimeProviderBridgeKind,
    body: &[u8],
) -> ProviderTransformResult {
    provider_translator(kind.provider_id()).transform_stream_event(ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        body.to_vec(),
    ))
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_stream_text_delta_event(
    kind: RuntimeProviderBridgeKind,
    upstream_value: &serde_json::Value,
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
) -> Option<(String, serde_json::Value)> {
    let mut event =
        runtime_provider_stream_base_event(kind, upstream_value, "response.output_text.delta")?;
    let object = event.1.as_object_mut()?;
    object.insert(
        "sequence_number".to_string(),
        serde_json::Value::from(sequence_number),
    );
    object.insert(
        "created_at".to_string(),
        serde_json::Value::from(created_at),
    );
    object.insert(
        "response_id".to_string(),
        serde_json::Value::String(response_id.to_string()),
    );
    Some(event)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_stream_function_call_arguments_delta_event(
    kind: RuntimeProviderBridgeKind,
    upstream_value: &serde_json::Value,
    sequence_number: u64,
) -> Option<(String, serde_json::Value)> {
    let mut event = runtime_provider_stream_base_event(
        kind,
        upstream_value,
        "response.function_call_arguments.delta",
    )?;
    event.1.as_object_mut()?.insert(
        "sequence_number".to_string(),
        serde_json::Value::from(sequence_number),
    );
    Some(event)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_stream_reasoning_summary_text_delta_event(
    kind: RuntimeProviderBridgeKind,
    upstream_value: &serde_json::Value,
    sequence_number: u64,
    response_id: &str,
    summary_index: u64,
) -> Option<(String, serde_json::Value)> {
    let mut event = runtime_provider_stream_base_event(
        kind,
        upstream_value,
        "response.reasoning_summary_text.delta",
    )?;
    let object = event.1.as_object_mut()?;
    object.insert(
        "sequence_number".to_string(),
        serde_json::Value::from(sequence_number),
    );
    object.insert(
        "response_id".to_string(),
        serde_json::Value::String(response_id.to_string()),
    );
    object.insert(
        "summary_index".to_string(),
        serde_json::Value::from(summary_index),
    );
    Some(event)
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_log_stream_conformance(
    shared: &crate::RuntimeRotationProxyShared,
    request_id: u64,
    kind: RuntimeProviderBridgeKind,
    result: &ProviderTransformResult,
) {
    runtime_provider_log_conformance(
        shared,
        request_id,
        kind,
        result,
        "local_rewrite_provider_conformance_stream",
    );
}

fn runtime_provider_stream_base_event(
    kind: RuntimeProviderBridgeKind,
    upstream_value: &serde_json::Value,
    expected_event_name: &str,
) -> Option<(String, serde_json::Value)> {
    let body = format!("data: {upstream_value}\n\n");
    let result = runtime_provider_stream_event_conformance_result(kind, body.as_bytes());
    if !matches!(result.loss, ProviderTransformLoss::Lossless) {
        return None;
    }
    let body = result.body.as_ref()?;
    let event = String::from_utf8_lossy(body);
    let event_name = event
        .strip_prefix("event: ")?
        .split_once('\n')?
        .0
        .trim()
        .to_string();
    if event_name != expected_event_name {
        return None;
    }
    let payload = event
        .split_once("data: ")?
        .1
        .trim()
        .strip_suffix('\n')
        .unwrap_or(event.split_once("data: ")?.1.trim());
    let data = serde_json::from_str::<serde_json::Value>(payload).ok()?;
    Some((event_name, data))
}

fn runtime_provider_log_conformance(
    shared: &crate::RuntimeRotationProxyShared,
    request_id: u64,
    kind: RuntimeProviderBridgeKind,
    result: &ProviderTransformResult,
    event_name: &'static str,
) {
    let Some((loss, reason)) = runtime_provider_loss_log_fields(&result.loss) else {
        return;
    };
    crate::runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            event_name,
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("provider", runtime_provider_label(kind)),
                runtime_proxy_log_field("endpoint", result.endpoint.label()),
                runtime_proxy_log_field("loss", loss),
                runtime_proxy_log_field("reason", reason),
            ],
        ),
    );
}

pub(super) fn runtime_provider_loss_log_fields(
    loss: &ProviderTransformLoss,
) -> Option<(&'static str, &str)> {
    match loss {
        ProviderTransformLoss::Lossless => None,
        ProviderTransformLoss::DegradedButSafe { reason, .. } => Some(("degraded", reason)),
        ProviderTransformLoss::Rejected { reason } => Some(("rejected", reason)),
        ProviderTransformLoss::UnsupportedUpstream { reason } => Some(("unsupported", reason)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conformance_headers_allowlist_only_continuation_metadata() {
        let headers = runtime_provider_conformance_headers(&[
            (
                "Authorization".to_string(),
                "Bearer conformance-secret".to_string(),
            ),
            ("X-Codex-Turn-State".to_string(), "turn-a".to_string()),
            ("SESSION_ID".to_string(), "session-a".to_string()),
            ("X-Request-Id".to_string(), "request-a".to_string()),
        ]);

        assert_eq!(headers.len(), 2);
        assert_eq!(
            headers.get("x-codex-turn-state").map(String::as_str),
            Some("turn-a")
        );
        assert_eq!(
            headers.get("session_id").map(String::as_str),
            Some("session-a")
        );
        assert!(
            !headers
                .values()
                .any(|value| value.contains("conformance-secret"))
        );
    }
}
