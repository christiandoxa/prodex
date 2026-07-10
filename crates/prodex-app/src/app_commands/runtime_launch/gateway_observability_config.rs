use super::*;
use std::{env, path::PathBuf};

pub(crate) fn gateway_observability_config(
    paths: &AppPaths,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<RuntimeGatewayObservabilityConfig> {
    let mut sinks = Vec::new();
    for sink in &policy.observability.sinks {
        if !gateway_exact_policy_identifier(sink) {
            bail!("gateway.observability.sinks must be non-empty strings without whitespace");
        }
        sinks.push(sink.clone());
    }
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
        .map(|value| {
            if value.trim().is_empty() {
                bail!("gateway.observability.jsonl_path cannot be empty");
            }
            let path = PathBuf::from(value);
            Ok(if path.is_absolute() {
                path
            } else {
                paths.root.join(path)
            })
        })
        .transpose()?;
    if jsonl_path.is_some() && !sinks.iter().any(|sink| sink.eq_ignore_ascii_case("jsonl")) {
        sinks.push("jsonl".to_string());
    }
    let http_endpoint = policy
        .observability
        .http_endpoint
        .as_deref()
        .map(|value| {
            gateway_observability_http_endpoint("gateway.observability.http_endpoint", value)
        })
        .transpose()?;
    if http_endpoint.is_some() && !sinks.iter().any(|sink| sink.eq_ignore_ascii_case("http")) {
        sinks.push("http".to_string());
    }
    let http_schema = policy
        .observability
        .http_schema
        .as_deref()
        .map(|value| {
            if !gateway_exact_policy_identifier(value) {
                bail!(
                    "gateway.observability.http_schema must be non-empty without whitespace"
                );
            }
            match value.to_ascii_lowercase().as_str() {
                "generic" | "otel" | "otlp" | "opentelemetry" | "datadog" | "langfuse" => {
                    Ok(value.to_ascii_lowercase())
                }
                _ => bail!(
                    "gateway.observability.http_schema must be one of generic, otel, otlp, datadog, langfuse"
                ),
            }
        })
        .transpose()?
        .unwrap_or_else(|| "generic".to_string());
    let http_bearer_token = match policy.observability.http_bearer_token_env.as_deref() {
        Some(env_name) if !gateway_exact_policy_identifier(env_name) => {
            bail!(
                "gateway.observability.http_bearer_token_env must be non-empty without whitespace"
            );
        }
        Some(env_name) => Some(gateway_secret_value_from_env(
            "gateway.observability.http_bearer_token_env",
            env_name,
        )?),
        None => None,
    };
    Ok(RuntimeGatewayObservabilityConfig {
        sinks,
        jsonl_path,
        http_endpoint,
        http_schema,
        http_bearer_token,
    })
}

fn gateway_observability_http_endpoint(field: &str, value: &str) -> Result<String> {
    if value.is_empty() {
        bail!("{field} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("{field} must not contain whitespace");
    }
    let parsed = reqwest::Url::parse(value)
        .with_context(|| format!("{field} must be an http(s) URL with host"))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        bail!("{field} must be an http(s) URL with host");
    }
    Ok(value.to_string())
}

fn gateway_secret_value_from_env(context: &str, env_name: &str) -> Result<String> {
    let value = env::var(env_name).with_context(|| format!("{context} requires {env_name}"))?;
    if value.is_empty() {
        bail!("{context} env {env_name} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("{context} env {env_name} must not contain whitespace");
    }
    Ok(value)
}

fn gateway_exact_policy_identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}
