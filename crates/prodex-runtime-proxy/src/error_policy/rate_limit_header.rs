use super::{
    RuntimeHttpErrorAction, RuntimeHttpErrorClass, RuntimeHttpErrorPhase, RuntimeHttpErrorPolicy,
    runtime_error_policy_match, runtime_http_error_policy,
};

/// Applies the official Codex rate-limit reached header after body classification.
pub fn runtime_http_error_policy_with_headers<'a>(
    status: u16,
    body: &[u8],
    headers: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    phase: RuntimeHttpErrorPhase,
) -> RuntimeHttpErrorPolicy {
    let body_policy = runtime_http_error_policy(status, body, phase);
    if status != 429 {
        return body_policy;
    }
    let reached_type = headers
        .into_iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("x-codex-rate-limit-reached-type"))
        .filter_map(|(_, value)| std::str::from_utf8(value).ok())
        .map(str::trim)
        .find(|value| !value.is_empty());
    let Some(reached_type) = reached_type else {
        return body_policy;
    };
    if reached_type.eq_ignore_ascii_case("rate_limit_reached") {
        return runtime_error_policy_match(
            RuntimeHttpErrorClass::RateLimited,
            RuntimeHttpErrorAction::RetryProfile,
            "rate_limited",
            "Upstream Codex profile is temporarily rate limited.".to_string(),
            phase,
        );
    }
    if [
        "workspace_owner_credits_depleted",
        "workspace_member_credits_depleted",
        "workspace_owner_usage_limit_reached",
        "workspace_member_usage_limit_reached",
    ]
    .into_iter()
    .any(|value| value.eq_ignore_ascii_case(reached_type))
    {
        runtime_error_policy_match(
            RuntimeHttpErrorClass::Quota,
            RuntimeHttpErrorAction::RotateProfile,
            "explicit_quota",
            "Upstream Codex account quota was exhausted.".to_string(),
            phase,
        )
    } else {
        body_policy
    }
}
