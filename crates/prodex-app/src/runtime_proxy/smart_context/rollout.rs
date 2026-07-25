//! Smart-context rollout and environment flag helpers.

use super::*;
pub(super) fn runtime_smart_context_rollout_decision(
    _request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    _route_kind: RuntimeRouteKind,
    _transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
) -> runtime_proxy_crate::SmartContextRolloutDecision {
    runtime_proxy_crate::smart_context_rollout_decision(
        runtime_proxy_crate::SmartContextRolloutDecisionInput {
            enabled: true,
            explicit_exact_mode: runtime_smart_context_exact_header(request),
            shadow_mode: shared.runtime_config.smart_context_shadow,
            canary_percent: shared.runtime_config.smart_context_canary_percent,
            stable_key: runtime_smart_context_rollout_stable_key(request, profile_name),
        },
    )
}

pub(super) fn runtime_smart_context_rollout_stable_key(
    request: &RuntimeProxyRequest,
    profile_name: Option<&str>,
) -> String {
    let turn_metadata = request
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-codex-turn-metadata"))
        .and_then(|(_, value)| serde_json::from_str::<serde_json::Value>(value).ok());
    let session_id = runtime_proxy_crate::runtime_request_explicit_session_id(request)
        .map(runtime_proxy_crate::RuntimeExplicitSessionId::into_string)
        .or_else(|| {
            turn_metadata
                .as_ref()
                .and_then(runtime_proxy_crate::runtime_request_session_id_from_value)
        });
    let workspace = turn_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("cwd"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    format!(
        "profile={}:session={}:workspace={}",
        profile_name.unwrap_or("-"),
        runtime_smart_context_rollout_scope_hash(session_id.as_deref()),
        runtime_smart_context_rollout_scope_hash(workspace),
    )
}

fn runtime_smart_context_rollout_scope_hash(value: Option<&str>) -> String {
    value
        .map(runtime_proxy_crate::smart_context_hash_text)
        .unwrap_or_else(|| "-".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(session_id: &str, cwd: &str, request_id: &str) -> RuntimeProxyRequest {
        RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/backend-api/codex/responses".to_string(),
            headers: vec![
                ("session_id".to_string(), session_id.to_string()),
                (
                    "x-codex-turn-metadata".to_string(),
                    serde_json::json!({"session_id": session_id, "cwd": cwd}).to_string(),
                ),
            ],
            body: serde_json::json!({"client_metadata": {"request_id": request_id}})
                .to_string()
                .into_bytes(),
        }
    }

    #[test]
    fn rollout_scope_is_sticky_without_request_entropy() {
        let first = runtime_smart_context_rollout_stable_key(
            &request("session-a", "/workspace/a", "request-1"),
            Some("profile-a"),
        );
        let next = runtime_smart_context_rollout_stable_key(
            &request("session-a", "/workspace/a", "request-2"),
            Some("profile-a"),
        );

        assert_eq!(first, next);
        assert!(!first.contains("request-"));
        assert!(!first.contains("/workspace/a"));
    }

    #[test]
    fn rollout_scope_separates_session_profile_and_workspace() {
        let base = request("session-a", "/workspace/a", "request-1");
        let base_key = runtime_smart_context_rollout_stable_key(&base, Some("profile-a"));

        assert_ne!(
            base_key,
            runtime_smart_context_rollout_stable_key(
                &request("session-b", "/workspace/a", "request-2"),
                Some("profile-a"),
            )
        );
        assert_ne!(
            base_key,
            runtime_smart_context_rollout_stable_key(
                &request("session-a", "/workspace/b", "request-2"),
                Some("profile-a"),
            )
        );
        assert_ne!(
            base_key,
            runtime_smart_context_rollout_stable_key(&base, Some("profile-b"))
        );
    }
}
