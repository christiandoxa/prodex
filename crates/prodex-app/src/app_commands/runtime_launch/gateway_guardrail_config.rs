use super::gateway_secret_config::GatewaySecretResolver;
use super::*;
use prodex_domain::SecretPurpose;

pub(crate) struct ResolvedGatewayGuardrailConfig {
    pub(crate) guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
    pub(crate) request_constraints: prodex_provider_core::ProviderRequestConstraintPolicy,
    pub(crate) webhook: RuntimeGatewayGuardrailWebhookConfig,
    pub(crate) presidio_redaction_enabled: bool,
}

#[cfg(test)]
pub(crate) fn resolve_gateway_guardrail_config(
    args: &GatewayArgs,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<ResolvedGatewayGuardrailConfig> {
    let resolver = GatewaySecretResolver::from_policy(&Default::default())?;
    resolve_gateway_guardrail_config_with_resolver(args, policy, &resolver)
}

pub(crate) fn resolve_gateway_guardrail_config_with_resolver(
    args: &GatewayArgs,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    resolver: &GatewaySecretResolver,
) -> Result<ResolvedGatewayGuardrailConfig> {
    let guardrails = gateway_guardrail_config(policy)?;
    let presidio_redaction_enabled = if args.presidio {
        true
    } else if args.no_presidio {
        false
    } else {
        policy.guardrails.presidio_redaction.unwrap_or(false)
    };
    Ok(ResolvedGatewayGuardrailConfig {
        guardrails,
        request_constraints: gateway_request_constraint_config(policy)?,
        webhook: gateway_guardrail_webhook_config_with_resolver(policy, resolver)?,
        presidio_redaction_enabled,
    })
}

pub(crate) fn gateway_guardrail_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<runtime_proxy_crate::RuntimeGatewayGuardrailConfig> {
    for model in &policy.guardrails.allowed_models {
        if !gateway_exact_policy_identifier(model) {
            bail!("gateway.guardrails.allowed_models must be non-empty strings without whitespace");
        }
    }
    validate_gateway_guardrail_keywords("blocked_keywords", &policy.guardrails.blocked_keywords)?;
    validate_gateway_guardrail_keywords(
        "blocked_output_keywords",
        &policy.guardrails.blocked_output_keywords,
    )?;
    Ok(runtime_proxy_crate::RuntimeGatewayGuardrailConfig {
        blocked_keywords: policy.guardrails.blocked_keywords.clone(),
        blocked_output_keywords: policy.guardrails.blocked_output_keywords.clone(),
        allowed_models: policy.guardrails.allowed_models.clone(),
        prompt_injection_detection: policy
            .guardrails
            .prompt_injection_detection
            .unwrap_or(false),
        pii_redaction: policy.guardrails.pii_redaction.unwrap_or(false),
    })
}

fn validate_gateway_guardrail_keywords(field: &str, keywords: &[String]) -> Result<()> {
    if keywords.len() > prodex_runtime_policy::MAX_GATEWAY_GUARDRAIL_KEYWORDS {
        bail!(
            "gateway.guardrails.{field} must contain at most {} entries",
            prodex_runtime_policy::MAX_GATEWAY_GUARDRAIL_KEYWORDS
        );
    }
    for keyword in keywords {
        if keyword.trim().is_empty() {
            bail!("gateway.guardrails.{field} entries cannot be blank");
        }
        if keyword.trim().len() > prodex_runtime_policy::MAX_GATEWAY_GUARDRAIL_KEYWORD_BYTES {
            bail!(
                "gateway.guardrails.{field} entries must be at most {} bytes",
                prodex_runtime_policy::MAX_GATEWAY_GUARDRAIL_KEYWORD_BYTES
            );
        }
    }
    Ok(())
}

fn gateway_request_constraint_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<prodex_provider_core::ProviderRequestConstraintPolicy> {
    Ok(prodex_provider_core::ProviderRequestConstraintPolicy {
        enabled: policy.request_constraints.enabled.unwrap_or(false),
        unknown_context: match policy.request_constraints.unknown_context.as_deref() {
            Some(value) => prodex_provider_core::ProviderUnknownContextPolicy::parse(value)
                .context("gateway.request_constraints.unknown_context is invalid")?,
            None => Default::default(),
        },
        safe_window_tokens: policy
            .request_constraints
            .safe_window_tokens
            .unwrap_or(prodex_provider_core::PROVIDER_REQUEST_SAFE_WINDOW_TOKENS_DEFAULT),
        oversized_output: match policy.request_constraints.oversized_output.as_deref() {
            Some(value) => prodex_provider_core::ProviderOversizedOutputPolicy::parse(value)
                .context("gateway.request_constraints.oversized_output is invalid")?,
            None => Default::default(),
        },
    })
}

#[cfg(test)]
pub(crate) fn gateway_guardrail_webhook_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<RuntimeGatewayGuardrailWebhookConfig> {
    let resolver = GatewaySecretResolver::from_policy(&Default::default())?;
    gateway_guardrail_webhook_config_with_resolver(policy, &resolver)
}

fn gateway_guardrail_webhook_config_with_resolver(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    resolver: &GatewaySecretResolver,
) -> Result<RuntimeGatewayGuardrailWebhookConfig> {
    let url = policy
        .guardrails
        .webhook_url
        .as_deref()
        .map(|value| {
            if value.is_empty() {
                bail!("gateway.guardrails.webhook_url cannot be empty");
            }
            if value.chars().any(char::is_whitespace) {
                bail!("gateway.guardrails.webhook_url must not contain whitespace");
            }
            let parsed = reqwest::Url::parse(value).with_context(
                || {
                    "gateway.guardrails.webhook_url must be an http(s) URL with host and no credentials, query, or fragment"
                },
            )?;
            if !matches!(parsed.scheme(), "http" | "https")
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.query().is_some()
                || parsed.fragment().is_some()
            {
                bail!(
                    "gateway.guardrails.webhook_url must be an http(s) URL with host and no credentials, query, or fragment"
                );
            }
            let Some(host) = parsed.host_str() else {
                bail!(
                    "gateway.guardrails.webhook_url must be an http(s) URL with host and no credentials, query, or fragment"
                );
            };
            if !policy.guardrails.webhook_host_allowlist.is_empty()
                && !policy
                    .guardrails
                    .webhook_host_allowlist
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(host))
            {
                bail!("gateway.guardrails.webhook_url host is not allowlisted");
            }
            Ok(value.to_string())
        })
        .transpose()?;
    let mut phases = Vec::new();
    for phase in &policy.guardrails.webhook_phases {
        if !gateway_exact_policy_identifier(phase) {
            bail!(
                "gateway.guardrails.webhook_phases must be pre/post/request/response without whitespace"
            );
        }
        phases.push(match phase.to_ascii_lowercase().as_str() {
            "pre" | "request" => "pre".to_string(),
            "post" | "response" => "post".to_string(),
            _ => {
                bail!("gateway.guardrails.webhook_phases must be pre/post/request/response")
            }
        });
    }
    if policy
        .guardrails
        .webhook_bearer_token_env
        .as_deref()
        .is_some_and(|value| !gateway_exact_policy_identifier(value))
    {
        bail!("gateway.guardrails.webhook_bearer_token_env must be non-empty without whitespace");
    }
    let bearer_token = gateway_guardrail_webhook_secret_with_resolver(policy, resolver)?;
    Ok(RuntimeGatewayGuardrailWebhookConfig {
        url,
        phases,
        bearer_token,
        fail_closed: policy.guardrails.webhook_fail_closed.unwrap_or(false),
    })
}

pub(crate) fn gateway_guardrail_webhook_secret_with_resolver(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    resolver: &GatewaySecretResolver,
) -> Result<Option<RuntimeGatewaySecret>> {
    let token_context = if policy.guardrails.webhook_bearer_token_ref.is_some() {
        "gateway.guardrails.webhook_bearer_token_ref"
    } else {
        "gateway.guardrails.webhook"
    };
    resolver.runtime_secret(
        token_context,
        policy.guardrails.webhook_bearer_token_ref.as_ref(),
        policy.guardrails.webhook_bearer_token_env.as_deref(),
        None,
        SecretPurpose::WebhookSigningSecret,
    )
}

fn gateway_exact_policy_identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod keyword_bound_tests {
    use super::gateway_guardrail_config;

    #[test]
    fn rejects_unbounded_keyword_values() {
        let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
        policy.guardrails.blocked_output_keywords =
            vec!["x".repeat(prodex_runtime_policy::MAX_GATEWAY_GUARDRAIL_KEYWORD_BYTES + 1)];
        assert!(gateway_guardrail_config(&policy).is_err());

        policy.guardrails.blocked_output_keywords =
            vec!["x".to_string(); prodex_runtime_policy::MAX_GATEWAY_GUARDRAIL_KEYWORDS + 1];
        assert!(gateway_guardrail_config(&policy).is_err());
    }
}
