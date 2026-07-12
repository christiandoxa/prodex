use super::gateway_secret_config::GatewaySecretResolver;
use super::*;
use prodex_domain::SecretPurpose;
use std::path::PathBuf;

#[cfg(test)]
pub(crate) fn gateway_observability_config(
    paths: &AppPaths,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<RuntimeGatewayObservabilityConfig> {
    let resolver = GatewaySecretResolver::from_policy(&Default::default())?;
    gateway_observability_config_with_resolver(paths, policy, &resolver)
}

pub(crate) fn gateway_observability_config_with_resolver(
    paths: &AppPaths,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    resolver: &GatewaySecretResolver,
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
    let http_bearer_token = gateway_observability_secret_with_resolver(policy, resolver)?;
    Ok(RuntimeGatewayObservabilityConfig {
        sinks,
        jsonl_path,
        http_endpoint,
        http_schema,
        http_bearer_token,
    })
}

pub(crate) fn gateway_observability_secret_with_resolver(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    resolver: &GatewaySecretResolver,
) -> Result<Option<RuntimeGatewaySecret>> {
    let token_context = if policy.observability.http_bearer_token_ref.is_some() {
        "gateway.observability.http_bearer_token_ref"
    } else {
        "gateway.observability.http_bearer_token_env"
    };
    resolver.runtime_secret(
        token_context,
        policy.observability.http_bearer_token_ref.as_ref(),
        policy.observability.http_bearer_token_env.as_deref(),
        None,
        SecretPurpose::TelemetryExportCredential,
    )
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

fn gateway_exact_policy_identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}
