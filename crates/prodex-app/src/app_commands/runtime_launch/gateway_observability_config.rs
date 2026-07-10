use super::*;
use std::{env, path::PathBuf};

pub(crate) fn gateway_observability_config(
    paths: &AppPaths,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<RuntimeGatewayObservabilityConfig> {
    let mut sinks = policy
        .observability
        .sinks
        .iter()
        .map(|sink| sink.trim().to_string())
        .filter(|sink| !sink.is_empty())
        .collect::<Vec<_>>();
    if !sinks
        .iter()
        .any(|sink| sink.eq_ignore_ascii_case("runtime-log") || sink.eq_ignore_ascii_case("log"))
    {
        sinks.push("runtime-log".to_string());
    }
    let jsonl_path = policy
        .observability
        .jsonl_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                path
            } else {
                paths.root.join(path)
            }
        });
    if jsonl_path.is_some() && !sinks.iter().any(|sink| sink.eq_ignore_ascii_case("jsonl")) {
        sinks.push("jsonl".to_string());
    }
    let http_endpoint = policy
        .observability
        .http_endpoint
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if http_endpoint.is_some() && !sinks.iter().any(|sink| sink.eq_ignore_ascii_case("http")) {
        sinks.push("http".to_string());
    }
    let http_schema = policy
        .observability
        .http_schema
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("generic")
        .to_ascii_lowercase();
    let http_bearer_token = policy
        .observability
        .http_bearer_token_env
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|env_name| {
            env::var(env_name)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        });
    Ok(RuntimeGatewayObservabilityConfig {
        sinks,
        jsonl_path,
        http_endpoint,
        http_schema,
        http_bearer_token,
    })
}
