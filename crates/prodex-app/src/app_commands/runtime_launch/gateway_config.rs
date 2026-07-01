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
    state: &AppState,
    args: &GatewayArgs,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<ResolvedGatewayLaunchConfig> {
    let provider = args
        .provider
        .or_else(|| policy.provider.as_deref().and_then(gateway_policy_provider));
    let provider_options = gateway_provider_options(state, provider, args.api_key.as_deref())?;
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

    let listen_addr = match args.listen.as_deref().or(policy.listen_addr.as_deref()) {
        Some(value) if gateway_exact_policy_identifier(value) => value.to_string(),
        Some(_) => bail!("gateway.listen_addr must be non-empty without whitespace"),
        None => "127.0.0.1:4000".to_string(),
    };
    gateway_validate_listen_auth(&listen_addr, auth_required)?;

    let provider_id = gateway_provider_catalog_id(provider);
    let route_aliases = policy
        .route_aliases
        .iter()
        .map(|alias| -> Result<_> {
            let strategy = match alias.strategy.as_deref() {
                Some(value) => runtime_proxy_crate::RuntimeGatewayRouteStrategy::parse(value)
                    .with_context(|| {
                        format!("gateway route alias '{}' strategy is invalid", alias.alias)
                    })?,
                None => Default::default(),
            };
            let models = alias
                .models
                .iter()
                .filter(|model| gateway_exact_policy_identifier(model))
                .cloned()
                .collect::<Vec<_>>();
            Ok(runtime_proxy_crate::RuntimeGatewayRouteAlias {
                alias: if gateway_exact_policy_identifier(&alias.alias) {
                    alias.alias.clone()
                } else {
                    String::new()
                },
                model_metrics: gateway_route_alias_model_metrics(
                    provider_id,
                    &models,
                    &alias.model_metrics,
                ),
                models,
                strategy,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let guardrails = gateway_guardrail_config(policy);
    let presidio_redaction_enabled = if args.presidio {
        true
    } else if args.no_presidio {
        false
    } else {
        policy.guardrails.presidio_redaction.unwrap_or(false)
    };
    let call_id_header = gateway_call_id_header_config(policy)?;

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
pub(super) fn gateway_guardrail_webhook_config(
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

pub(super) fn gateway_call_id_header_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<String> {
    Ok(policy
        .observability
        .call_id_header
        .as_deref()
        .map(|value| {
            gateway_exact_policy_identifier(value)
                .then_some(value)
                .context(
                    "gateway.observability.call_id_header must be non-empty without whitespace",
                )
        })
        .transpose()?
        .unwrap_or("x-prodex-call-id")
        .to_string())
}

pub(super) fn gateway_guardrail_config(
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
pub(super) fn gateway_state_store_config(
    paths: &AppPaths,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<RuntimeGatewayStateStore> {
    let backend = match policy.state.backend.as_deref() {
        Some(value) if gateway_exact_policy_identifier(value) => value,
        Some(_) => bail!("gateway.state.backend must be non-empty without whitespace"),
        None => "file",
    };
    match backend.to_ascii_lowercase().as_str() {
        "file" => Ok(RuntimeGatewayStateStore::file(paths)),
        "sqlite" => {
            let configured = policy
                .state
                .sqlite_path
                .as_deref()
                .filter(|value| !value.trim().is_empty())
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
                .map(|value| {
                    gateway_exact_policy_identifier(value)
                        .then_some(value)
                        .context(
                            "gateway.state.postgres_url_env must be non-empty without whitespace",
                        )
                })
                .transpose()?
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
                .map(|value| {
                    gateway_exact_policy_identifier(value)
                        .then_some(value)
                        .context("gateway.state.redis_url_env must be non-empty without whitespace")
                })
                .transpose()?
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
pub(super) fn gateway_admin_tokens_config(
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
        if !gateway_exact_policy_identifier(&configured.token_env) {
            continue;
        }
        let Some(token) = env::var(&configured.token_env)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let role = match configured.role.as_deref() {
            Some(role) => RuntimeGatewayAdminRole::parse(role).with_context(|| {
                format!(
                    "gateway.admin_tokens role for {:?} must be admin or viewer",
                    configured.name
                )
            })?,
            None => RuntimeGatewayAdminRole::Viewer,
        };
        tokens.push(RuntimeGatewayAdminToken {
            name: if gateway_exact_policy_identifier(&configured.name) {
                configured.name.clone()
            } else {
                String::new()
            },
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(&token),
            role,
            tenant_id: gateway_optional_policy_string(configured.tenant_id.as_deref()),
            team_id: gateway_optional_policy_string(configured.team_id.as_deref()),
            project_id: gateway_optional_policy_string(configured.project_id.as_deref()),
            user_id: gateway_optional_policy_string(configured.user_id.as_deref()),
            budget_id: gateway_optional_policy_string(configured.budget_id.as_deref()),
            allowed_key_prefixes: configured
                .allowed_key_prefixes
                .iter()
                .filter(|prefix| !prefix.is_empty() && !prefix.chars().any(char::is_whitespace))
                .cloned()
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
        .filter(|value| gateway_exact_policy_identifier(value))
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
    Ok(RuntimeGatewaySsoConfig {
        proxy_token_hash,
        require_tenant: policy.sso.require_tenant.unwrap_or(false),
        token_header: gateway_sso_header(&policy.sso.token_header, "x-prodex-sso-token"),
        user_header: gateway_sso_header(&policy.sso.user_header, "x-prodex-sso-user"),
        role_header: gateway_sso_header(&policy.sso.role_header, "x-prodex-sso-role"),
        tenant_header: gateway_sso_header(&policy.sso.tenant_header, "x-prodex-sso-tenant"),
        key_prefixes_header: gateway_sso_header(
            &policy.sso.key_prefixes_header,
            "x-prodex-sso-key-prefixes",
        ),
        oidc: gateway_sso_oidc_config(policy)?,
    })
}

fn gateway_sso_oidc_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<Option<RuntimeGatewayOidcConfig>> {
    let Some(issuer) = policy
        .sso
        .oidc_issuer
        .as_deref()
        .filter(|value| gateway_exact_policy_identifier(value))
    else {
        return Ok(None);
    };
    let audience = policy
        .sso
        .oidc_audience
        .as_deref()
        .filter(|value| gateway_exact_policy_identifier(value))
        .context("gateway.sso.oidc_audience is required when oidc_issuer is set")?;
    Ok(Some(RuntimeGatewayOidcConfig {
        issuer: issuer.to_string(),
        audience: audience.to_string(),
        jwks_url: policy
            .sso
            .oidc_jwks_url
            .as_deref()
            .filter(|value| gateway_exact_policy_identifier(value))
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
        .filter(|value| gateway_exact_policy_identifier(value))
        .unwrap_or(default)
        .to_string()
}

pub(super) fn gateway_observability_config(
    paths: &AppPaths,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<RuntimeGatewayObservabilityConfig> {
    let mut sinks = policy
        .observability
        .sinks
        .iter()
        .filter(|sink| gateway_exact_policy_identifier(sink))
        .cloned()
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
        .filter(|value| !value.trim().is_empty())
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
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if http_endpoint.is_some() && !sinks.iter().any(|sink| sink.eq_ignore_ascii_case("http")) {
        sinks.push("http".to_string());
    }
    let http_schema = policy
        .observability
        .http_schema
        .as_deref()
        .map(|value| {
            gateway_exact_policy_identifier(value)
                .then_some(value)
                .context("gateway.observability.http_schema must be non-empty without whitespace")
        })
        .transpose()?
        .unwrap_or("generic")
        .to_ascii_lowercase();
    let http_bearer_token = policy
        .observability
        .http_bearer_token_env
        .as_deref()
        .filter(|value| gateway_exact_policy_identifier(value))
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

pub(super) fn gateway_virtual_keys_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
) -> Result<Vec<runtime_proxy_crate::RuntimeGatewayVirtualKey>> {
    policy
        .virtual_keys
        .iter()
        .map(|key| {
            let token_env = key.token_env.as_str();
            if !gateway_exact_policy_identifier(token_env) {
                bail!("gateway virtual key '{}' token_env is invalid", key.name);
            }
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
                name: if gateway_exact_policy_identifier(&key.name) {
                    key.name.clone()
                } else {
                    String::new()
                },
                tenant_id: gateway_optional_policy_string(key.tenant_id.as_deref()),
                team_id: gateway_optional_policy_string(key.team_id.as_deref()),
                project_id: gateway_optional_policy_string(key.project_id.as_deref()),
                user_id: gateway_optional_policy_string(key.user_id.as_deref()),
                budget_id: gateway_optional_policy_string(key.budget_id.as_deref()),
                token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(&token),
                allowed_models: key
                    .allowed_models
                    .iter()
                    .filter(|model| gateway_exact_policy_identifier(model))
                    .cloned()
                    .collect(),
                budget_microusd: key.budget_usd.map(gateway_budget_usd_to_microusd),
                request_budget: key.request_budget,
                rpm_limit: key.rpm_limit,
                tpm_limit: key.tpm_limit,
            })
        })
        .collect()
}

fn gateway_optional_policy_string(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| gateway_exact_policy_identifier(value))
        .map(str::to_string)
}

fn gateway_exact_policy_identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}

fn gateway_budget_usd_to_microusd(value: f64) -> u64 {
    (value * 1_000_000.0).round().clamp(1.0, u64::MAX as f64) as u64
}

fn gateway_provider_catalog_id(
    provider: Option<SuperExternalProvider>,
) -> Option<prodex_provider_core::ProviderId> {
    match provider {
        Some(SuperExternalProvider::Anthropic) => Some(prodex_provider_core::ProviderId::Anthropic),
        Some(SuperExternalProvider::Copilot) => Some(prodex_provider_core::ProviderId::Copilot),
        Some(SuperExternalProvider::DeepSeek) => Some(prodex_provider_core::ProviderId::DeepSeek),
        Some(SuperExternalProvider::Gemini) => Some(prodex_provider_core::ProviderId::Gemini),
        Some(SuperExternalProvider::Kiro) => Some(prodex_provider_core::ProviderId::Kiro),
        None => Some(prodex_provider_core::ProviderId::OpenAi),
    }
}

fn gateway_route_alias_model_metrics(
    provider: Option<prodex_provider_core::ProviderId>,
    models: &[String],
    configured: &[prodex_runtime_policy::RuntimePolicyGatewayRouteModelMetrics],
) -> BTreeMap<String, runtime_proxy_crate::RuntimeGatewayRouteModelMetrics> {
    let mut metrics = BTreeMap::new();
    for model in models {
        if let Some(provider) = provider {
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
    }
    for metric in configured {
        if !gateway_exact_policy_identifier(&metric.model) {
            continue;
        }
        let entry = metrics.entry(metric.model.clone()).or_default();
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
    if !gateway_exact_policy_identifier(value) {
        return None;
    }
    match value.to_ascii_lowercase().as_str() {
        "anthropic" | "claude" => Some(SuperExternalProvider::Anthropic),
        "copilot" | "github-copilot" | "github_copilot" => Some(SuperExternalProvider::Copilot),
        "deepseek" => Some(SuperExternalProvider::DeepSeek),
        "gemini" => Some(SuperExternalProvider::Gemini),
        "kiro" => Some(SuperExternalProvider::Kiro),
        _ => None,
    }
}

pub(super) fn gateway_upstream_base_url(
    args: &GatewayArgs,
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    provider: Option<SuperExternalProvider>,
) -> Result<String> {
    let raw = args
        .base_url
        .as_deref()
        .or(policy.base_url.as_deref())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| provider.map(|provider| provider.default_base_url().to_string()))
        .or_else(|| {
            env::var("OPENAI_BASE_URL")
                .ok()
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    gateway_normalize_upstream_base_url(&raw, provider)
}

pub(super) fn gateway_normalize_upstream_base_url(
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

fn gateway_provider_options(
    state: &AppState,
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
        Some(SuperExternalProvider::Kiro) => {
            super::providers::runtime_kiro_gateway_profile_auth(state)
                .map(|auth| RuntimeLocalRewriteProviderOptions::Kiro { auth })
                .context(
                    "gateway kiro provider requires an imported Kiro profile from `prodex profile import kiro`",
                )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KIRO_MODEL_CATALOG_FILE, ProfileEntry, ProfileProvider};
    use std::collections::BTreeMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "prodex-gateway-config-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("temp dir should exist");
        dir
    }

    #[test]
    fn gateway_policy_provider_accepts_kiro() {
        assert_eq!(
            gateway_policy_provider("kiro"),
            Some(SuperExternalProvider::Kiro)
        );
    }

    #[test]
    fn gateway_provider_options_accepts_imported_kiro_profile() {
        let root = temp_dir("kiro");
        let codex_home = root.join("kiro-home");
        fs::create_dir_all(&codex_home).expect("codex home should exist");
        fs::write(
            codex_home.join(KIRO_MODEL_CATALOG_FILE),
            serde_json::json!({
                "models": [
                    { "id": "claude-sonnet-4", "name": "claude-sonnet-4", "owned_by": "kiro-cli" }
                ]
            })
            .to_string(),
        )
        .expect("kiro model catalog should be written");
        let state = AppState {
            active_profile: Some("kiro-main".to_string()),
            profiles: BTreeMap::from([(
                "kiro-main".to_string(),
                ProfileEntry {
                    codex_home: codex_home.clone(),
                    managed: true,
                    email: Some("kiro@example.com".to_string()),
                    provider: ProfileProvider::Kiro {
                        auth_key: "kiro-key".to_string(),
                        auth_kind: Some("builder-id".to_string()),
                        profile_arn: None,
                        profile_name: None,
                        start_url: None,
                        region: Some("us-east-1".to_string()),
                    },
                },
            )]),
            ..Default::default()
        };
        let options = gateway_provider_options(&state, Some(SuperExternalProvider::Kiro), None)
            .expect("kiro gateway should use imported profile");
        let RuntimeLocalRewriteProviderOptions::Kiro { auth } = options else {
            panic!("expected kiro provider options");
        };
        assert_eq!(auth.profile_name, "kiro-main");
        assert_eq!(auth.codex_home, codex_home);
        assert_eq!(auth.model_catalog.len(), 1);
    }

    #[test]
    fn gateway_kiro_route_alias_metrics_do_not_infer_openai_costs() {
        let metrics = gateway_route_alias_model_metrics(
            gateway_provider_catalog_id(Some(SuperExternalProvider::Kiro)),
            &[String::from("gpt-5.4")],
            &[],
        );
        assert!(metrics.is_empty());
    }
}
