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
    let guardrails = runtime_proxy_crate::RuntimeGatewayGuardrailConfig {
        blocked_keywords: policy
            .guardrails
            .blocked_keywords
            .iter()
            .map(|keyword| keyword.trim().to_string())
            .filter(|keyword| !keyword.is_empty())
            .collect(),
        blocked_output_keywords: policy
            .guardrails
            .blocked_output_keywords
            .iter()
            .map(|keyword| keyword.trim().to_string())
            .filter(|keyword| !keyword.is_empty())
            .collect(),
        allowed_models: policy
            .guardrails
            .allowed_models
            .iter()
            .map(|model| model.trim().to_string())
            .filter(|model| !model.is_empty())
            .collect(),
        prompt_injection_detection: policy
            .guardrails
            .prompt_injection_detection
            .unwrap_or(false),
        pii_redaction: policy.guardrails.pii_redaction.unwrap_or(false),
    };
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

fn gateway_guardrail_webhook_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> RuntimeGatewayGuardrailWebhookConfig {
    let url = policy
        .guardrails
        .webhook_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let phases = policy
        .guardrails
        .webhook_phases
        .iter()
        .map(|phase| phase.trim().to_ascii_lowercase())
        .filter(|phase| !phase.is_empty())
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
        .map(str::trim)
        .filter(|value| !value.is_empty())
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
