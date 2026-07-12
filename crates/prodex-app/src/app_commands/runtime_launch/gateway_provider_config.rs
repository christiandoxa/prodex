use super::gateway_secret_config::GatewaySecretResolver;
use super::*;
use prodex_domain::SecretPurpose;
#[cfg(test)]
use std::env;

pub(crate) struct ResolvedGatewayProviderConfig {
    pub(crate) provider: Option<SuperExternalProvider>,
    pub(crate) provider_options: RuntimeLocalRewriteProviderOptions,
    pub(crate) provider_credential: Option<RuntimeProjectedProviderCredential>,
    pub(crate) upstream_base_url: String,
}

pub(crate) struct ResolvedGatewayProviderCredentials {
    pub(crate) provider: Option<SuperExternalProvider>,
    pub(crate) provider_options: RuntimeLocalRewriteProviderOptions,
    pub(crate) provider_credential: Option<RuntimeProjectedProviderCredential>,
}

pub(crate) fn resolve_gateway_provider_config_with_resolver(
    state: &AppState,
    args: &GatewayArgs,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    resolver: &GatewaySecretResolver,
    runtime_config: &RuntimeConfig,
) -> Result<ResolvedGatewayProviderConfig> {
    let credentials = resolve_gateway_provider_credentials_with_resolver(
        state,
        args,
        policy,
        resolver,
        runtime_config,
    )?;
    let upstream_base_url = runtime_config
        .gateway
        .launch
        .upstream_base_url()
        .context("gateway startup base URL is unavailable")?
        .to_string();
    Ok(ResolvedGatewayProviderConfig {
        provider: credentials.provider,
        provider_options: credentials.provider_options,
        provider_credential: credentials.provider_credential,
        upstream_base_url,
    })
}

pub(crate) fn resolve_gateway_provider_credentials_with_resolver(
    state: &AppState,
    args: &GatewayArgs,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    resolver: &GatewaySecretResolver,
    runtime_config: &RuntimeConfig,
) -> Result<ResolvedGatewayProviderCredentials> {
    let provider = match args.provider {
        Some(provider) => Some(provider),
        None => match policy.provider.as_deref() {
            Some(value) => Some(gateway_policy_provider(value)?),
            None => None,
        },
    };
    if resolver.production() && matches!(provider, Some(SuperExternalProvider::Kiro)) {
        bail!(
            "gateway Kiro profile credentials are forbidden in production; use a provider with projected credentials"
        );
    }
    let provider_credential = resolver.projected_provider_credential(
        "gateway provider API key",
        policy.provider_api_key_ref.as_ref(),
        args.api_key.as_deref(),
    )?;
    let provider_options = if provider_credential.is_some() {
        gateway_projected_provider_options(provider, runtime_config)?
    } else {
        let api_keys = gateway_provider_api_keys(args, provider, resolver, runtime_config)?;
        gateway_provider_options(state, provider, api_keys, runtime_config)?
    };
    Ok(ResolvedGatewayProviderCredentials {
        provider,
        provider_options,
        provider_credential,
    })
}

fn gateway_projected_provider_options(
    provider: Option<SuperExternalProvider>,
    runtime_config: &RuntimeConfig,
) -> Result<RuntimeLocalRewriteProviderOptions> {
    match provider {
        Some(SuperExternalProvider::Anthropic) => {
            Ok(RuntimeLocalRewriteProviderOptions::Anthropic {
                auth: RuntimeAnthropicProviderAuth::Projected,
            })
        }
        Some(SuperExternalProvider::Copilot) => Ok(RuntimeLocalRewriteProviderOptions::Copilot {
            auth: RuntimeCopilotProviderAuth::Projected,
        }),
        Some(SuperExternalProvider::DeepSeek) => {
            let deepseek = runtime_config
                .gateway
                .launch
                .deepseek()
                .context("DeepSeek gateway startup configuration is unavailable")?;
            Ok(RuntimeLocalRewriteProviderOptions::DeepSeek {
                api_keys: Vec::new(),
                strict_tools: deepseek.strict_tools,
                beta_base_url: deepseek.beta_base_url.clone(),
                web_search_mode: deepseek.web_search_mode,
            })
        }
        Some(SuperExternalProvider::Gemini) => Ok(RuntimeLocalRewriteProviderOptions::Gemini {
            auth: RuntimeGeminiProviderAuth::Projected,
            thinking_budget_tokens: None,
            model_resolution: runtime_config
                .gateway
                .launch
                .gemini_model_resolution()
                .context("Gemini gateway startup configuration is unavailable")?
                .clone(),
        }),
        Some(SuperExternalProvider::Kiro) => {
            bail!("gateway Kiro does not accept a projected provider API key")
        }
        None => Ok(RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: Vec::new(),
        }),
    }
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

#[cfg(test)]
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

#[cfg(test)]
pub(crate) fn gateway_normalize_upstream_base_url(
    value: &str,
    provider: Option<SuperExternalProvider>,
) -> Result<String> {
    if value.chars().any(char::is_whitespace) {
        bail!("gateway --base-url must not contain whitespace");
    }
    let trimmed = value.trim_end_matches('/').to_string();
    let parsed = reqwest::Url::parse(&trimmed).context(
        "gateway --base-url must be an http(s) URL with host and no credentials, query, or fragment",
    )?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        bail!(
            "gateway --base-url must be an http(s) URL with host and no credentials, query, or fragment"
        );
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
    api_keys: Option<Vec<String>>,
    runtime_config: &RuntimeConfig,
) -> Result<RuntimeLocalRewriteProviderOptions> {
    match provider {
        Some(SuperExternalProvider::Anthropic) => {
            api_keys
                .map(|api_keys| RuntimeLocalRewriteProviderOptions::Anthropic {
                    auth: RuntimeAnthropicProviderAuth::ApiKeys { api_keys },
                })
                .context("gateway anthropic provider requires --api-key or ANTHROPIC_API_KEY(S)")
        }
        Some(SuperExternalProvider::Copilot) => {
            api_keys
                .map(|api_keys| RuntimeLocalRewriteProviderOptions::Copilot {
                    auth: RuntimeCopilotProviderAuth::ApiKeys { api_keys },
                })
                .context("gateway copilot provider requires --api-key or GITHUB_COPILOT_API_KEY(S)")
        }
        Some(SuperExternalProvider::DeepSeek) => {
            let api_keys = api_keys
                .context("gateway deepseek provider requires --api-key or DEEPSEEK_API_KEY(S)")?;
            let deepseek = runtime_config
                .gateway
                .launch
                .deepseek()
                .context("DeepSeek gateway startup configuration is unavailable")?;
            Ok(RuntimeLocalRewriteProviderOptions::DeepSeek {
                api_keys,
                strict_tools: deepseek.strict_tools,
                beta_base_url: deepseek.beta_base_url.clone(),
                web_search_mode: deepseek.web_search_mode,
            })
        }
        Some(SuperExternalProvider::Gemini) => {
            let api_keys = api_keys.context(
                "gateway gemini provider requires --api-key or GEMINI_API_KEY(S) / GOOGLE_API_KEY(S)",
            )?;
            Ok(RuntimeLocalRewriteProviderOptions::Gemini {
                auth: RuntimeGeminiProviderAuth::ApiKeys { api_keys },
                thinking_budget_tokens: None,
                model_resolution: runtime_config
                    .gateway
                    .launch
                    .gemini_model_resolution()
                    .context("Gemini gateway startup configuration is unavailable")?
                    .clone(),
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
            api_keys: api_keys.unwrap_or_default(),
        }),
    }
}

fn gateway_provider_api_keys(
    args: &GatewayArgs,
    provider: Option<SuperExternalProvider>,
    resolver: &GatewaySecretResolver,
    runtime_config: &RuntimeConfig,
) -> Result<Option<Vec<String>>> {
    if let Some(value) = args.api_key.as_deref() {
        return resolver
            .resolve(
                "gateway provider API key",
                None,
                None,
                Some(value),
                SecretPurpose::ProviderCredential,
            )
            .map(|value| value.map(|value| vec![value]));
    }
    let (plural, single): (&[&'static str], &[&'static str]) = match provider {
        None => (&["OPENAI_API_KEYS"], &["OPENAI_API_KEY"]),
        Some(SuperExternalProvider::Anthropic) => (&["ANTHROPIC_API_KEYS"], &["ANTHROPIC_API_KEY"]),
        Some(SuperExternalProvider::Copilot) => {
            (&["GITHUB_COPILOT_API_KEYS"], &["GITHUB_COPILOT_API_KEY"])
        }
        Some(SuperExternalProvider::DeepSeek) => (&["DEEPSEEK_API_KEYS"], &["DEEPSEEK_API_KEY"]),
        Some(SuperExternalProvider::Gemini) => (
            &["GEMINI_API_KEYS", "GOOGLE_API_KEYS"],
            &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        ),
        Some(SuperExternalProvider::Kiro) => return Ok(None),
    };
    let source = runtime_config
        .gateway
        .launch
        .first_secret_env(plural, true)
        .or_else(|| {
            runtime_config
                .gateway
                .launch
                .first_secret_env(single, false)
        });
    let Some(source) = source else {
        return Ok(None);
    };
    let value = resolver.resolve_environment_raw("gateway provider API key", source.name)?;
    if source.list {
        return gateway_api_keys_from_list(&value)
            .with_context(|| format!("{} cannot be empty", source.name))
            .map(Some);
    }
    if value.is_empty() {
        bail!("{} cannot be empty", source.name);
    }
    if value.chars().any(char::is_whitespace) {
        bail!("{} must not contain whitespace", source.name);
    }
    Ok(Some(vec![value]))
}

#[cfg(test)]
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
