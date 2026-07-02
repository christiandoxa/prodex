use super::*;
use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
};

#[path = "gateway_config_helpers.rs"]
mod gateway_config_helpers;
pub(super) use gateway_config_helpers::gateway_api_keys_from_list;
use gateway_config_helpers::{
    gateway_budget_usd_to_microusd, gateway_optional_policy_string, gateway_validate_listen_auth,
};

pub(super) struct ResolvedGatewayLaunchConfig {
    pub(super) provider_name: Option<&'static str>,
    pub(super) upstream_base_url: String,
    pub(super) provider_options: RuntimeLocalRewriteProviderOptions,
    pub(super) auth_token_hash: Option<runtime_proxy_crate::LocalBridgeBearerTokenHash>,
    pub(super) auth_required: bool,
    pub(super) listen_addr: String,
    pub(super) admin_tokens: Vec<RuntimeGatewayAdminToken>,
    pub(super) sso: RuntimeGatewaySsoConfig,
    pub(super) state_store: RuntimeGatewayStateStore,
    pub(super) virtual_keys: Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>,
    pub(super) route_aliases: Vec<runtime_proxy_crate::RuntimeGatewayRouteAlias>,
    pub(super) guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig,
    pub(super) guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig,
    pub(super) call_id_header: String,
    pub(super) observability: RuntimeGatewayObservabilityConfig,
    pub(super) presidio_redaction_enabled: bool,
}

pub(super) fn resolve_gateway_launch_config(
    paths: &AppPaths,
    args: &GatewayArgs,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<ResolvedGatewayLaunchConfig> {
    let provider = args
        .provider
        .or_else(|| policy.provider.as_deref().and_then(gateway_policy_provider));
    let provider_options = gateway_provider_options(provider, args.api_key.as_deref())?;
    let auth_token = args
        .auth_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            env::var("PRODEX_GATEWAY_TOKEN")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        });
    let admin_tokens = gateway_admin_tokens_config(auth_token.as_deref(), policy)?;
    if policy.require_auth == Some(true) && auth_token.is_none() && policy.virtual_keys.is_empty() {
        bail!(
            "gateway auth is required by policy.toml; set --auth-token, PRODEX_GATEWAY_TOKEN, or [[gateway.virtual_keys]]; [[gateway.admin_tokens]] only protects admin endpoints"
        );
    }
    let virtual_keys = gateway_virtual_keys_config(policy)?;
    let auth_required = auth_token.is_some() || !virtual_keys.is_empty();
    if policy.require_auth == Some(true) && !auth_required {
        bail!("gateway auth is required by policy.toml; configured virtual key env vars are empty");
    }
    if auth_required
        && matches!(
            &provider_options,
            RuntimeLocalRewriteProviderOptions::OpenAiResponses { api_keys }
                if api_keys.is_empty()
        )
    {
        bail!(
            "OpenAI-compatible gateway auth requires a separate upstream key; set --api-key, OPENAI_API_KEY, or OPENAI_API_KEYS"
        );
    }

    let listen_addr = args
        .listen
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            policy
                .listen_addr
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "127.0.0.1:4000".to_string());
    gateway_validate_listen_auth(&listen_addr, auth_required)?;

    let provider_id = gateway_provider_catalog_id(provider);
    let route_aliases = policy
        .route_aliases
        .iter()
        .map(|alias| {
            let strategy = alias
                .strategy
                .as_deref()
                .and_then(runtime_proxy_crate::RuntimeGatewayRouteStrategy::parse)
                .unwrap_or_default();
            let models = alias
                .models
                .iter()
                .map(|model| model.trim().to_string())
                .filter(|model| !model.is_empty())
                .collect::<Vec<_>>();
            runtime_proxy_crate::RuntimeGatewayRouteAlias {
                alias: alias.alias.trim().to_string(),
                model_metrics: gateway_route_alias_model_metrics(
                    provider_id,
                    &models,
                    &alias.model_metrics,
                ),
                models,
                strategy,
            }
        })
        .collect::<Vec<_>>();
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
    let call_id_header = policy
        .observability
        .call_id_header
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("x-prodex-call-id")
        .to_string();

    Ok(ResolvedGatewayLaunchConfig {
        provider_name: provider.map(SuperExternalProvider::as_str),
        upstream_base_url: gateway_upstream_base_url(args, policy, provider)?,
        provider_options,
        auth_token_hash: auth_token
            .as_deref()
            .map(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token),
        auth_required,
        listen_addr,
        admin_tokens,
        sso: gateway_sso_config(policy)?,
        state_store: gateway_state_store_config(paths, policy)?,
        virtual_keys,
        route_aliases,
        guardrails,
        guardrail_webhook: gateway_guardrail_webhook_config(policy),
        call_id_header,
        observability: gateway_observability_config(paths, policy)?,
        presidio_redaction_enabled,
    })
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
pub(super) fn gateway_state_store_config(
    paths: &AppPaths,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<RuntimeGatewayStateStore> {
    match policy
        .state
        .backend
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("file")
        .to_ascii_lowercase()
        .as_str()
    {
        "file" => Ok(RuntimeGatewayStateStore::file(paths)),
        "sqlite" => {
            let configured = policy
                .state
                .sqlite_path
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("gateway-state.sqlite");
            let path = PathBuf::from(configured);
            let path = if path.is_absolute() {
                path
            } else {
                paths.root.join(path)
            };
            Ok(RuntimeGatewayStateStore::sqlite(path))
        }
        "postgres" => {
            let env_name = policy
                .state
                .postgres_url_env
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("PRODEX_GATEWAY_POSTGRES_URL");
            let url = env::var(env_name)
                .with_context(|| format!("gateway.state.backend=postgres requires {env_name}"))?
                .trim()
                .to_string();
            if url.is_empty() {
                bail!("gateway.state.backend=postgres env {env_name} cannot be empty");
            }
            Ok(RuntimeGatewayStateStore::postgres(
                env_name.to_string(),
                url,
            ))
        }
        "redis" => {
            let env_name = policy
                .state
                .redis_url_env
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("PRODEX_GATEWAY_REDIS_URL");
            let url = env::var(env_name)
                .with_context(|| format!("gateway.state.backend=redis requires {env_name}"))?
                .trim()
                .to_string();
            if url.is_empty() {
                bail!("gateway.state.backend=redis env {env_name} cannot be empty");
            }
            Ok(RuntimeGatewayStateStore::redis(env_name.to_string(), url))
        }
        other => {
            bail!("gateway.state.backend must be file, sqlite, postgres, or redis, got {other:?}")
        }
    }
}
fn gateway_admin_tokens_config(
    legacy_admin_token: Option<&str>,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<Vec<RuntimeGatewayAdminToken>> {
    let mut tokens = Vec::new();
    if let Some(token) = legacy_admin_token
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        tokens.push(RuntimeGatewayAdminToken {
            name: "default-admin".to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(token),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        });
    }
    for configured in &policy.admin_tokens {
        let Some(token) = env::var(&configured.token_env)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let role = configured
            .role
            .as_deref()
            .and_then(RuntimeGatewayAdminRole::parse)
            .unwrap_or(RuntimeGatewayAdminRole::Admin);
        tokens.push(RuntimeGatewayAdminToken {
            name: configured.name.trim().to_string(),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(&token),
            role,
            tenant_id: configured
                .tenant_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            team_id: gateway_optional_policy_string(configured.team_id.as_deref()),
            project_id: gateway_optional_policy_string(configured.project_id.as_deref()),
            user_id: gateway_optional_policy_string(configured.user_id.as_deref()),
            budget_id: gateway_optional_policy_string(configured.budget_id.as_deref()),
            allowed_key_prefixes: configured
                .allowed_key_prefixes
                .iter()
                .map(|prefix| prefix.trim().to_string())
                .filter(|prefix| !prefix.is_empty())
                .collect(),
        });
    }
    Ok(tokens)
}
pub(super) fn gateway_sso_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<RuntimeGatewaySsoConfig> {
    let proxy_token_hash = match policy
        .sso
        .proxy_token_env
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(env_name) => {
            let token = env::var(env_name)
                .with_context(|| format!("gateway.sso.proxy_token_env requires {env_name}"))?
                .trim()
                .to_string();
            if token.is_empty() {
                bail!("gateway.sso.proxy_token_env env {env_name} cannot be empty");
            }
            Some(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                &token,
            ))
        }
        None => None,
    };
    let default_role = policy
        .sso
        .default_role
        .as_deref()
        .and_then(RuntimeGatewayAdminRole::parse)
        .unwrap_or(RuntimeGatewayAdminRole::Admin);
    Ok(RuntimeGatewaySsoConfig {
        proxy_token_hash,
        token_header: gateway_sso_header(&policy.sso.token_header, "x-prodex-sso-token"),
        user_header: gateway_sso_header(&policy.sso.user_header, "x-prodex-sso-user"),
        role_header: gateway_sso_header(&policy.sso.role_header, "x-prodex-sso-role"),
        tenant_header: gateway_sso_header(&policy.sso.tenant_header, "x-prodex-sso-tenant"),
        key_prefixes_header: gateway_sso_header(
            &policy.sso.key_prefixes_header,
            "x-prodex-sso-key-prefixes",
        ),
        oidc: gateway_sso_oidc_config(policy)?,
        default_role,
    })
}

fn gateway_sso_oidc_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<Option<RuntimeGatewayOidcConfig>> {
    let Some(issuer) = policy
        .sso
        .oidc_issuer
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let audience = policy
        .sso
        .oidc_audience
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("gateway.sso.oidc_audience is required when oidc_issuer is set")?;
    Ok(Some(RuntimeGatewayOidcConfig {
        issuer: issuer.to_string(),
        audience: audience.to_string(),
        jwks_url: policy
            .sso
            .oidc_jwks_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        user_claim: gateway_sso_header(&policy.sso.oidc_user_claim, "email"),
        role_claim: gateway_sso_header(&policy.sso.oidc_role_claim, "prodex_role"),
        tenant_claim: gateway_sso_header(&policy.sso.oidc_tenant_claim, "prodex_tenant"),
        key_prefixes_claim: gateway_sso_header(
            &policy.sso.oidc_key_prefixes_claim,
            "prodex_key_prefixes",
        ),
    }))
}

fn gateway_sso_header(value: &Option<String>, default: &str) -> String {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_string()
}

fn gateway_observability_config(
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

fn gateway_virtual_keys_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>> {
    policy
        .virtual_keys
        .iter()
        .map(|key| {
            let token_env = key.token_env.trim();
            let token = env::var(token_env)
                .with_context(|| {
                    format!("gateway virtual key '{}' requires {token_env}", key.name)
                })?
                .trim()
                .to_string();
            if token.is_empty() {
                bail!(
                    "gateway virtual key '{}' env {token_env} cannot be empty",
                    key.name
                );
            }
            Ok(runtime_proxy_crate::RuntimeGatewayVirtualKey {
                name: key.name.trim().to_string(),
                tenant_id: key
                    .tenant_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
                team_id: gateway_optional_policy_string(key.team_id.as_deref()),
                project_id: gateway_optional_policy_string(key.project_id.as_deref()),
                user_id: gateway_optional_policy_string(key.user_id.as_deref()),
                budget_id: gateway_optional_policy_string(key.budget_id.as_deref()),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(&token),
                allowed_models: key
                    .allowed_models
                    .iter()
                    .map(|model| model.trim().to_string())
                    .filter(|model| !model.is_empty())
                    .collect(),
                budget_microusd: key.budget_usd.map(gateway_budget_usd_to_microusd),
                request_budget: key.request_budget,
                rpm_limit: key.rpm_limit,
                tpm_limit: key.tpm_limit,
            })
        })
        .collect()
}

fn gateway_provider_catalog_id(
    provider: Option<SuperExternalProvider>,
) -> prodex_provider_core::ProviderId {
    match provider {
        Some(SuperExternalProvider::Anthropic) => prodex_provider_core::ProviderId::Anthropic,
        Some(SuperExternalProvider::Copilot) => prodex_provider_core::ProviderId::Copilot,
        Some(SuperExternalProvider::DeepSeek) => prodex_provider_core::ProviderId::DeepSeek,
        Some(SuperExternalProvider::Gemini) => prodex_provider_core::ProviderId::Gemini,
        None => prodex_provider_core::ProviderId::OpenAi,
    }
}

fn gateway_route_alias_model_metrics(
    provider: prodex_provider_core::ProviderId,
    models: &[String],
    configured: &[prodex_runtime_policy::RuntimePolicyGatewayRouteModelMetrics],
) -> BTreeMap<String, runtime_proxy_crate::RuntimeGatewayRouteModelMetrics> {
    let mut metrics = BTreeMap::new();
    for model in models {
        let cost = prodex_provider_core::provider_model_cost(provider, model);
        if cost.any() {
            metrics.insert(
                model.clone(),
                runtime_proxy_crate::RuntimeGatewayRouteModelMetrics {
                    input_cost_per_million_microusd: cost.input_cost_per_million_microusd,
                    output_cost_per_million_microusd: cost.output_cost_per_million_microusd,
                    ..runtime_proxy_crate::RuntimeGatewayRouteModelMetrics::default()
                },
            );
        }
    }
    for metric in configured {
        let model = metric.model.trim().to_string();
        if model.is_empty() {
            continue;
        }
        let entry = metrics.entry(model).or_default();
        if metric.input_cost_per_million_microusd.is_some() {
            entry.input_cost_per_million_microusd = metric.input_cost_per_million_microusd;
        }
        if metric.output_cost_per_million_microusd.is_some() {
            entry.output_cost_per_million_microusd = metric.output_cost_per_million_microusd;
        }
        if metric.latency_ms.is_some() {
            entry.latency_ms = metric.latency_ms;
        }
        if metric.rpm_limit.is_some() {
            entry.rpm_limit = metric.rpm_limit;
        }
        if metric.tpm_limit.is_some() {
            entry.tpm_limit = metric.tpm_limit;
        }
    }
    metrics
}

fn gateway_policy_provider(value: &str) -> Option<SuperExternalProvider> {
    match value.trim().to_ascii_lowercase().as_str() {
        "anthropic" | "claude" => Some(SuperExternalProvider::Anthropic),
        "copilot" | "github-copilot" | "github_copilot" => Some(SuperExternalProvider::Copilot),
        "deepseek" => Some(SuperExternalProvider::DeepSeek),
        "gemini" => Some(SuperExternalProvider::Gemini),
        _ => None,
    }
}

fn gateway_upstream_base_url(
    args: &GatewayArgs,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    provider: Option<SuperExternalProvider>,
) -> Result<String> {
    let raw = args
        .base_url
        .as_deref()
        .or(policy.base_url.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| provider.map(|provider| provider.default_base_url().to_string()))
        .or_else(|| {
            env::var("OPENAI_BASE_URL")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    gateway_normalize_upstream_base_url(&raw, provider)
}

fn gateway_normalize_upstream_base_url(
    value: &str,
    provider: Option<SuperExternalProvider>,
) -> Result<String> {
    let trimmed = value.trim().trim_end_matches('/').to_string();
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

fn gateway_provider_options(
    provider: Option<SuperExternalProvider>,
    api_key: Option<&str>,
) -> Result<RuntimeLocalRewriteProviderOptions> {
    match provider {
        Some(SuperExternalProvider::Anthropic) => {
            runtime_anthropic_api_keys_from_request_or_env(api_key)
                .map(|api_keys| RuntimeLocalRewriteProviderOptions::Anthropic {
                    auth: RuntimeAnthropicProviderAuth::ApiKeys { api_keys },
                })
                .context("gateway anthropic provider requires --api-key or ANTHROPIC_API_KEY(S)")
        }
        Some(SuperExternalProvider::Copilot) => {
            runtime_copilot_api_keys_from_request_or_env(api_key)
                .map(|api_keys| RuntimeLocalRewriteProviderOptions::Copilot {
                    auth: RuntimeCopilotProviderAuth::ApiKeys { api_keys },
                })
                .context("gateway copilot provider requires --api-key or GITHUB_COPILOT_API_KEY(S)")
        }
        Some(SuperExternalProvider::DeepSeek) => {
            runtime_deepseek_api_keys_from_request_or_env(api_key)
                .map(|api_keys| RuntimeLocalRewriteProviderOptions::DeepSeek {
                    api_keys,
                    strict_tools: runtime_deepseek_strict_tools_enabled(Path::new("")),
                    beta_base_url: runtime_deepseek_beta_base_url(Path::new("")),
                    web_search_mode: runtime_deepseek_web_search_mode(Path::new("")),
                })
                .context("gateway deepseek provider requires --api-key or DEEPSEEK_API_KEY(S)")
        }
        Some(SuperExternalProvider::Gemini) => {
            let api_keys = runtime_gemini_api_keys_from_request_or_env(api_key).context(
                "gateway gemini provider requires --api-key or GEMINI_API_KEY(S) / GOOGLE_API_KEY(S)",
            )?;
            Ok(RuntimeLocalRewriteProviderOptions::Gemini {
                auth: RuntimeGeminiProviderAuth::ApiKeys { api_keys },
                thinking_budget_tokens: None,
                model_resolution: RuntimeGeminiModelResolution::from_current_settings(),
            })
        }
        None => Ok(RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: gateway_openai_api_keys(api_key),
        }),
    }
}

fn gateway_openai_api_keys(value: Option<&str>) -> Vec<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| vec![value.to_string()])
        .or_else(|| {
            env::var("OPENAI_API_KEYS")
                .ok()
                .and_then(|value| gateway_api_keys_from_list(&value))
                .or_else(|| {
                    env::var("OPENAI_API_KEY")
                        .ok()
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                        .map(|value| vec![value])
                })
        })
        .unwrap_or_default()
}
