use super::*;
use std::{env, path::Path};

pub(crate) struct ResolvedGatewayProviderConfig {
    pub(crate) provider: Option<SuperExternalProvider>,
    pub(crate) provider_options: RuntimeLocalRewriteProviderOptions,
    pub(crate) upstream_base_url: String,
}

pub(crate) fn resolve_gateway_provider_config(
    state: &AppState,
    args: &GatewayArgs,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<ResolvedGatewayProviderConfig> {
    let provider = match args.provider {
        Some(provider) => Some(provider),
        None => match policy.provider.as_deref() {
            Some(value) => Some(gateway_policy_provider(value)?),
            None => None,
        },
    };
    let provider_options = gateway_provider_options(state, provider, args.api_key.as_deref())?;
    let upstream_base_url = gateway_upstream_base_url(args, policy, provider)?;
    Ok(ResolvedGatewayProviderConfig {
        provider,
        provider_options,
        upstream_base_url,
    })
}

pub(crate) fn gateway_policy_provider(value: &str) -> Result<SuperExternalProvider> {
    if !gateway_exact_policy_identifier(value) {
        bail!("gateway.provider must be non-empty without whitespace");
    }
    match value.to_ascii_lowercase().as_str() {
        "anthropic" | "claude" => Ok(SuperExternalProvider::Anthropic),
        "copilot" | "github-copilot" | "github_copilot" => Ok(SuperExternalProvider::Copilot),
        "deepseek" => Ok(SuperExternalProvider::DeepSeek),
        "gemini" => Ok(SuperExternalProvider::Gemini),
        "kiro" => Ok(SuperExternalProvider::Kiro),
        _ => bail!("gateway.provider is not a supported provider preset"),
    }
}

pub(crate) fn gateway_upstream_base_url(
    args: &GatewayArgs,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    provider: Option<SuperExternalProvider>,
) -> Result<String> {
    let raw = if let Some(value) = args.base_url.as_deref() {
        if value.is_empty() {
            bail!("gateway --base-url cannot be empty");
        }
        value.to_string()
    } else if let Some(value) = policy.base_url.as_deref() {
        if value.is_empty() {
            bail!("gateway.base_url cannot be empty");
        }
        value.to_string()
    } else if let Some(provider) = provider {
        provider.default_base_url().to_string()
    } else if let Ok(value) = env::var("OPENAI_BASE_URL") {
        if value.is_empty() {
            bail!("OPENAI_BASE_URL cannot be empty");
        }
        value
    } else {
        "https://api.openai.com/v1".to_string()
    };
    gateway_normalize_upstream_base_url(&raw, provider)
}

pub(crate) fn gateway_normalize_upstream_base_url(
    value: &str,
    provider: Option<SuperExternalProvider>,
) -> Result<String> {
    if value.chars().any(char::is_whitespace) {
        bail!("gateway --base-url must not contain whitespace");
    }
    let trimmed = value.trim_end_matches('/').to_string();
    let parsed = reqwest::Url::parse(&trimmed)
        .with_context(|| format!("invalid gateway --base-url {trimmed:?}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        bail!("gateway --base-url must use http or https");
    }
    if parsed.host_str().is_none() {
        bail!("gateway --base-url must include a host");
    }
    if provider.is_none() && parsed.path().trim_matches('/').is_empty() {
        Ok(format!("{trimmed}/v1"))
    } else {
        Ok(trimmed)
    }
}

pub(crate) fn gateway_provider_options(
    state: &AppState,
    provider: Option<SuperExternalProvider>,
    api_key: Option<&str>,
) -> Result<RuntimeLocalRewriteProviderOptions> {
    match provider {
        Some(SuperExternalProvider::Anthropic) => {
            runtime_anthropic_api_keys_from_request_or_env(api_key)?
                .map(|api_keys| RuntimeLocalRewriteProviderOptions::Anthropic {
                    auth: RuntimeAnthropicProviderAuth::ApiKeys { api_keys },
                })
                .context("gateway anthropic provider requires --api-key or ANTHROPIC_API_KEY(S)")
        }
        Some(SuperExternalProvider::Copilot) => {
            runtime_copilot_api_keys_from_request_or_env(api_key)?
                .map(|api_keys| RuntimeLocalRewriteProviderOptions::Copilot {
                    auth: RuntimeCopilotProviderAuth::ApiKeys { api_keys },
                })
                .context("gateway copilot provider requires --api-key or GITHUB_COPILOT_API_KEY(S)")
        }
        Some(SuperExternalProvider::DeepSeek) => {
            let api_keys = runtime_deepseek_api_keys_from_request_or_env(api_key)?
                .context("gateway deepseek provider requires --api-key or DEEPSEEK_API_KEY(S)")?;
            Ok(RuntimeLocalRewriteProviderOptions::DeepSeek {
                api_keys,
                strict_tools: runtime_deepseek_strict_tools_enabled(Path::new(""))?,
                beta_base_url: runtime_deepseek_beta_base_url(Path::new(""))?,
                web_search_mode: runtime_deepseek_web_search_mode(Path::new(""))?,
            })
        }
        Some(SuperExternalProvider::Gemini) => {
            let api_keys = runtime_gemini_api_keys_from_request_or_env(api_key)?.context(
                "gateway gemini provider requires --api-key or GEMINI_API_KEY(S) / GOOGLE_API_KEY(S)",
            )?;
            Ok(RuntimeLocalRewriteProviderOptions::Gemini {
                auth: RuntimeGeminiProviderAuth::ApiKeys { api_keys },
                thinking_budget_tokens: None,
                model_resolution: RuntimeGeminiModelResolution::from_current_settings(),
            })
        }
        Some(SuperExternalProvider::Kiro) => {
            super::providers::runtime_kiro_gateway_profile_auth(state)
                .map(|auth| RuntimeLocalRewriteProviderOptions::Kiro { auth })
                .context(
                    "gateway kiro provider requires an imported Kiro profile from `prodex profile import kiro`",
                )
        }
        None => Ok(RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: gateway_openai_api_keys(api_key)?,
        }),
    }
}

pub(crate) fn gateway_openai_api_keys(value: Option<&str>) -> Result<Vec<String>> {
    if let Some(value) = value {
        if value.is_empty() {
            bail!("gateway --api-key cannot be empty");
        }
        if value.chars().any(char::is_whitespace) {
            bail!("gateway --api-key must not contain whitespace");
        }
        return Ok(vec![value.to_string()]);
    }
    if let Ok(value) = env::var("OPENAI_API_KEYS") {
        return gateway_api_keys_from_list(&value).context("OPENAI_API_KEYS cannot be empty");
    }
    if let Ok(value) = env::var("OPENAI_API_KEY") {
        if value.is_empty() {
            bail!("OPENAI_API_KEY cannot be empty");
        }
        if value.chars().any(char::is_whitespace) {
            bail!("OPENAI_API_KEY must not contain whitespace");
        }
        return Ok(vec![value]);
    }
    Ok(Vec::new())
}

fn gateway_exact_policy_identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}
