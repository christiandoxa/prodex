use super::super::*;
use super::apply::{
    RuntimeProbeAuthObservation, apply_runtime_profile_probe_observation,
    apply_runtime_profile_probe_result_with_timeout,
};
use redaction::redaction_redact_secret_like_text;

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
        observation: Option<RuntimeProbeAuthObservation<'_>>,
    ) -> Result<()> {
        match self {
            Self::Inline { .. } => apply_runtime_profile_probe_observation(
                shared,
                profile_name,
                auth,
                result,
                observation,
            ),
            Self::Queued { apply_timeout } => apply_runtime_profile_probe_result_with_timeout(
                shared,
                profile_name,
                auth,
                result,
                observation,
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
                            "{}_error profile={} error={}",
                            context,
                            profile_name,
                            runtime_probe_refresh_state_update_error(&err)
                        )
                    } else {
                        format!("{}_ok profile={profile_name}", context)
                    },
                ),
                Err(err) => runtime_proxy_log(
                    shared,
                    format!(
                        "{}_error profile={} error={}",
                        context,
                        profile_name,
                        runtime_probe_refresh_error_text(err)
                    ),
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
                                        runtime_probe_refresh_state_update_error(&err),
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
                                runtime_proxy_log_field(
                                    "error",
                                    runtime_probe_refresh_error_text(err),
                                ),
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
    authenticated_identity: Option<UsageAuth>,
    started_at: i64,
}

impl RuntimeProbeRefreshAttempt {
    pub(super) fn collect(
        codex_home: &Path,
        upstream_base_url: &str,
        upstream_no_proxy: bool,
    ) -> Self {
        let started_at = Local::now().timestamp();
        let auth = read_auth_summary(codex_home);
        let fetched = if auth.quota_compatible {
            crate::fetch_usage_with_authenticated_identity(
                codex_home,
                Some(upstream_base_url),
                upstream_no_proxy,
            )
            .map_err(|err| err.to_string())
        } else {
            Err("auth mode is not quota-compatible".to_string())
        };
        let (result, authenticated_identity) = match fetched {
            Ok((usage, identity)) => (Ok(usage), Some(identity)),
            Err(error) => (Err(error), None),
        };
        Self {
            auth,
            result,
            authenticated_identity,
            started_at,
        }
    }

    pub(super) fn execute(
        self,
        mode: RuntimeProbeExecutionMode<'_>,
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
        queued_at: Instant,
    ) {
        let observation =
            self.authenticated_identity
                .as_ref()
                .map(|auth| RuntimeProbeAuthObservation {
                    auth,
                    started_at: self.started_at,
                });
        let apply_result = mode.apply(
            shared,
            profile_name,
            self.auth,
            self.result.clone(),
            observation,
        );
        mode.log_completion(shared, profile_name, queued_at, &self.result, apply_result);
    }
}

pub(crate) fn runtime_probe_refresh_error_text(err: &str) -> String {
    redaction_redact_secret_like_text(err)
}

pub(crate) fn runtime_probe_refresh_state_update_error(err: &anyhow::Error) -> String {
    format!(
        "state_update:{}",
        redaction_redact_secret_like_text(&format!("{err:#}"))
    )
}
