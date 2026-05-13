use super::super::*;
use super::apply::{
    apply_runtime_profile_probe_result, apply_runtime_profile_probe_result_with_timeout,
};

pub(super) enum RuntimeProbeExecutionMode<'a> {
    Inline { context: &'a str },
    Queued { apply_timeout: Duration },
}

impl RuntimeProbeExecutionMode<'_> {
    fn apply(
        &self,
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
        auth: AuthSummary,
        result: std::result::Result<UsageResponse, String>,
    ) -> Result<()> {
        match self {
            Self::Inline { .. } => {
                apply_runtime_profile_probe_result(shared, profile_name, auth, result)
            }
            Self::Queued { apply_timeout } => apply_runtime_profile_probe_result_with_timeout(
                shared,
                profile_name,
                auth,
                result,
                *apply_timeout,
            ),
        }
    }

    fn log_completion(
        &self,
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
        queued_at: Instant,
        result: &std::result::Result<UsageResponse, String>,
        apply_result: Result<()>,
    ) {
        match self {
            Self::Inline { context } => match result {
                Ok(_) => runtime_proxy_log(
                    shared,
                    if let Err(err) = apply_result {
                        format!(
                            "{}_error profile={} error=state_update:{err:#}",
                            context, profile_name
                        )
                    } else {
                        format!("{}_ok profile={profile_name}", context)
                    },
                ),
                Err(err) => runtime_proxy_log(
                    shared,
                    format!("{}_error profile={} error={err}", context, profile_name),
                ),
            },
            Self::Queued { .. } => {
                let lag_ms = queued_at.elapsed().as_millis();
                match result {
                    Ok(_) => runtime_proxy_log(
                        shared,
                        if let Err(err) = apply_result {
                            runtime_proxy_structured_log_message(
                                "profile_probe_refresh_error",
                                [
                                    runtime_proxy_log_field("profile", profile_name),
                                    runtime_proxy_log_field("lag_ms", lag_ms.to_string()),
                                    runtime_proxy_log_field(
                                        "error",
                                        format!("state_update:{err:#}"),
                                    ),
                                ],
                            )
                        } else {
                            runtime_proxy_structured_log_message(
                                "profile_probe_refresh_ok",
                                [
                                    runtime_proxy_log_field("profile", profile_name),
                                    runtime_proxy_log_field("lag_ms", lag_ms.to_string()),
                                ],
                            )
                        },
                    ),
                    Err(err) => runtime_proxy_log(
                        shared,
                        runtime_proxy_structured_log_message(
                            "profile_probe_refresh_error",
                            [
                                runtime_proxy_log_field("profile", profile_name),
                                runtime_proxy_log_field("lag_ms", lag_ms.to_string()),
                                runtime_proxy_log_field("error", err.as_str()),
                            ],
                        ),
                    ),
                }
            }
        }
    }
}

pub(super) struct RuntimeProbeRefreshAttempt {
    auth: AuthSummary,
    result: std::result::Result<UsageResponse, String>,
}

impl RuntimeProbeRefreshAttempt {
    pub(super) fn collect(
        codex_home: &Path,
        upstream_base_url: &str,
        upstream_no_proxy: bool,
    ) -> Self {
        let auth = read_auth_summary(codex_home);
        let result = if auth.quota_compatible {
            fetch_usage_with_proxy_policy(codex_home, Some(upstream_base_url), upstream_no_proxy)
                .map_err(|err| err.to_string())
        } else {
            Err("auth mode is not quota-compatible".to_string())
        };
        Self { auth, result }
    }

    pub(super) fn execute(
        self,
        mode: RuntimeProbeExecutionMode<'_>,
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
        queued_at: Instant,
    ) {
        let apply_result = mode.apply(shared, profile_name, self.auth, self.result.clone());
        mode.log_completion(shared, profile_name, queued_at, &self.result, apply_result);
    }
}

#[cfg(test)]
pub(crate) fn execute_runtime_probe_attempt_inline_for_test(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &str,
    result: std::result::Result<UsageResponse, String>,
) {
    RuntimeProbeRefreshAttempt {
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result,
    }
    .execute(
        RuntimeProbeExecutionMode::Inline { context },
        shared,
        profile_name,
        Instant::now(),
    );
}

#[cfg(test)]
pub(crate) fn execute_runtime_probe_attempt_queued_for_test(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    result: std::result::Result<UsageResponse, String>,
    apply_timeout: Duration,
    queued_at: Instant,
) {
    RuntimeProbeRefreshAttempt {
        auth: AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result,
    }
    .execute(
        RuntimeProbeExecutionMode::Queued { apply_timeout },
        shared,
        profile_name,
        queued_at,
    );
}
