use super::*;
use std::env;

pub(crate) struct ResolvedGatewayGuardrailConfig {
    pub(crate) guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
    pub(crate) webhook: RuntimeGatewayGuardrailWebhookConfig,
    pub(crate) presidio_redaction_enabled: bool,
}

pub(crate) fn resolve_gateway_guardrail_config(
    args: &GatewayArgs,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> ResolvedGatewayGuardrailConfig {
    let guardrails = gateway_guardrail_config(policy);
    let presidio_redaction_enabled = if args.presidio {
        true
    } else if args.no_presidio {
        false
    } else {
        policy.guardrails.presidio_redaction.unwrap_or(false)
    };
    ResolvedGatewayGuardrailConfig {
        guardrails,
        webhook: gateway_guardrail_webhook_config(policy),
        presidio_redaction_enabled,
    }
}

pub(crate) fn gateway_guardrail_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> runtime_proxy_crate::RuntimeGatewayGuardrailConfig {
    runtime_proxy_crate::RuntimeGatewayGuardrailConfig {
        blocked_keywords: policy
            .guardrails
            .blocked_keywords
            .iter()
            .filter(|keyword| !keyword.trim().is_empty())
            .cloned()
            .collect(),
        blocked_output_keywords: policy
            .guardrails
            .blocked_output_keywords
            .iter()
            .filter(|keyword| !keyword.trim().is_empty())
            .cloned()
            .collect(),
        allowed_models: policy
            .guardrails
            .allowed_models
            .iter()
            .filter(|model| gateway_exact_policy_identifier(model))
            .cloned()
            .collect(),
        prompt_injection_detection: policy
            .guardrails
            .prompt_injection_detection
            .unwrap_or(false),
        pii_redaction: policy.guardrails.pii_redaction.unwrap_or(false),
    }
}

pub(crate) fn gateway_guardrail_webhook_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> RuntimeGatewayGuardrailWebhookConfig {
    let url = policy
        .guardrails
        .webhook_url
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let phases = policy
        .guardrails
        .webhook_phases
        .iter()
        .filter(|phase| gateway_exact_policy_identifier(phase))
        .map(|phase| phase.to_ascii_lowercase())
        .map(|phase| match phase.as_str() {
            "request" => "pre".to_string(),
            "response" => "post".to_string(),
            _ => phase,
        })
        .collect();
    let bearer_token = policy
        .guardrails
        .webhook_bearer_token_env
        .as_deref()
        .filter(|value| gateway_exact_policy_identifier(value))
        .and_then(|env_name| {
            env::var(env_name)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        });
    RuntimeGatewayGuardrailWebhookConfig {
        url,
        phases,
        bearer_token,
        fail_closed: policy.guardrails.webhook_fail_closed.unwrap_or(false),
    }
}

fn gateway_exact_policy_identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}
