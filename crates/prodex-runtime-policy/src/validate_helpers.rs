use anyhow::{Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NumericRule {
    NonZero(u64),
    Range {
        value: u64,
        minimum: u64,
        maximum: u64,
    },
    LessOrEqual {
        value: u64,
        maximum: u64,
    },
}

pub(crate) fn failed_numeric_rules(rules: &[NumericRule]) -> Result<Vec<usize>> {
    #[cfg(feature = "mojo")]
    {
        let rules = rules
            .iter()
            .map(|rule| match *rule {
                NumericRule::NonZero(value) => prodex_mojo_core::policy::NumericRule {
                    kind: prodex_mojo_core::policy::POLICY_NUMERIC_NON_ZERO,
                    value,
                    minimum: 0,
                    maximum: u64::MAX,
                    related_value: 0,
                },
                NumericRule::Range {
                    value,
                    minimum,
                    maximum,
                } => prodex_mojo_core::policy::NumericRule {
                    kind: prodex_mojo_core::policy::POLICY_NUMERIC_RANGE,
                    value,
                    minimum,
                    maximum,
                    related_value: 0,
                },
                NumericRule::LessOrEqual { value, maximum } => {
                    prodex_mojo_core::policy::NumericRule {
                        kind: prodex_mojo_core::policy::POLICY_NUMERIC_RELATION_LE,
                        value,
                        minimum: 0,
                        maximum: 0,
                        related_value: maximum,
                    }
                }
            })
            .collect::<Vec<_>>();
        prodex_mojo_core::policy::validate_numeric_rules(&rules).map_err(|_| {
            anyhow::anyhow!("runtime policy numeric validation returned invalid output")
        })
    }
    #[cfg(not(feature = "mojo"))]
    {
        Ok(rules
            .iter()
            .enumerate()
            .filter_map(|(index, rule)| {
                let failed = match *rule {
                    NumericRule::NonZero(value) => value == 0,
                    NumericRule::Range {
                        value,
                        minimum,
                        maximum,
                    } => value < minimum || value > maximum,
                    NumericRule::LessOrEqual { value, maximum } => value > maximum,
                };
                failed.then_some(index)
            })
            .collect())
    }
}

pub(crate) fn validate_gateway_route_strategy(value: &str) -> Result<()> {
    if value.is_empty() {
        bail!("strategy cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("strategy must not contain whitespace");
    }
    match value.to_ascii_lowercase().as_str() {
        "fallback" | "ordered-fallback" | "ordered_fallback" | "round-robin" | "round_robin"
        | "rr" | "first" | "first-available" | "first_available" | "ordered" | "least-busy"
        | "least_busy" | "least-busy-model" | "least_busy_model" | "lowest-cost"
        | "lowest_cost" | "cost" | "cost-optimized" | "cost_optimized" | "lowest-latency"
        | "lowest_latency" | "latency" | "latency-optimized" | "latency_optimized" | "rpm"
        | "rpm-headroom" | "rpm_headroom" | "tpm" | "tpm-headroom" | "tpm_headroom" => Ok(()),
        _ => bail!(
            "strategy must be one of fallback, round-robin, first, least-busy, lowest-cost, lowest-latency, rpm, tpm"
        ),
    }
}

pub(crate) fn validate_gateway_observability_http_schema(value: &str) -> Result<()> {
    if value.is_empty() {
        bail!("schema cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("schema must not contain whitespace");
    }
    match value.to_ascii_lowercase().as_str() {
        "generic" | "otel" | "otlp" | "opentelemetry" | "datadog" | "langfuse" => Ok(()),
        _ => bail!("schema must be one of generic, otel, otlp, datadog, langfuse"),
    }
}

pub(crate) fn validate_gateway_state_backend(value: &str) -> Result<()> {
    if value.is_empty() {
        bail!("backend cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("backend must not contain whitespace");
    }
    match value.to_ascii_lowercase().as_str() {
        "file" | "sqlite" | "postgres" | "redis" => Ok(()),
        _ => bail!("backend must be one of file, sqlite, postgres, redis"),
    }
}

pub(crate) fn validate_gateway_admin_role(value: &str) -> Result<()> {
    if value.is_empty() {
        bail!("role cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("role must not contain whitespace");
    }
    match value.to_ascii_lowercase().as_str() {
        "admin" | "write" | "writer" | "viewer" | "read" | "readonly" | "read-only" => Ok(()),
        _ => bail!("role must be one of admin, viewer"),
    }
}

pub(crate) fn validate_gateway_guardrail_webhook_phase(value: &str) -> Result<()> {
    if value.is_empty() {
        bail!("phase cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("phase must not contain whitespace");
    }
    match value.to_ascii_lowercase().as_str() {
        "pre" | "request" | "post" | "response" => Ok(()),
        _ => bail!("phase must be one of pre, post"),
    }
}

pub(crate) fn gateway_observability_http_endpoint_has_http_host(value: &str) -> bool {
    let Some((scheme, rest)) = value.split_once("://") else {
        return false;
    };
    if !matches!(scheme, "http" | "https") {
        return false;
    }
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .trim();
    !host.is_empty() && !host.contains('@')
}
