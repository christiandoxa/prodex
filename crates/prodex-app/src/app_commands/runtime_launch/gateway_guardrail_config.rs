use super::gateway_secret_config::GatewaySecretResolver;
use super::*;
use prodex_domain::SecretPurpose;

pub(crate) struct ResolvedGatewayGuardrailConfig {
    pub(crate) guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
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
    for keyword in &policy.guardrails.blocked_keywords {
        if keyword.trim().is_empty() {
            bail!("gateway.guardrails.blocked_keywords entries cannot be blank");
        }
    }
    for keyword in &policy.guardrails.blocked_output_keywords {
        if keyword.trim().is_empty() {
            bail!("gateway.guardrails.blocked_output_keywords entries cannot be blank");
        }
    }
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
                || "gateway.guardrails.webhook_url must be an http(s) URL with host",
            )?;
            if !matches!(parsed.scheme(), "http" | "https")
                || parsed.host_str().is_none()
                || !parsed.username().is_empty()
                || parsed.password().is_some()
            {
                bail!("gateway.guardrails.webhook_url must be an http(s) URL with host");
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
    let token_context = if policy.guardrails.webhook_bearer_token_ref.is_some() {
        "gateway.guardrails.webhook_bearer_token_ref"
    } else {
        "gateway.guardrails.webhook"
    };
    let bearer_token = resolver.resolve(
        token_context,
        policy.guardrails.webhook_bearer_token_ref.as_ref(),
        policy.guardrails.webhook_bearer_token_env.as_deref(),
        None,
        SecretPurpose::WebhookSigningSecret,
    )?;
    Ok(RuntimeGatewayGuardrailWebhookConfig {
        url,
        phases,
        bearer_token,
        fail_closed: policy.guardrails.webhook_fail_closed.unwrap_or(false),
    })
}

fn gateway_exact_policy_identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}
