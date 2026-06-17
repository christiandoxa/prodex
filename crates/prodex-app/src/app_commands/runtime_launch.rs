use super::*;
mod preflight;
mod provider_names;
mod providers;
mod resume_repair;
use std::collections::BTreeMap;
use {preflight::*, provider_names::*, providers::*, resume_repair::*};

struct RunCommandStrategy {
    args: RunArgs,
    codex_args: Vec<OsString>,
    include_code_review: bool,
    mem_mode: Option<RuntimeMemTranscriptMode>,
    dry_run: bool,
    model_provider_override: Option<String>,
    profile_v2_name: Option<String>,
    model_context_window_tokens: Option<u64>,
    gemini_thinking_budget_tokens: Option<u64>,
    auto_external_provider: Option<SuperExternalProvider>,
    auto_external_provider_base_url: Option<String>,
    delete_session_id: Option<String>,
}

impl RunCommandStrategy {
    fn new(args: RunArgs) -> Result<Self> {
        let (mem_mode, codex_args) = runtime_mem_extract_mode_with_detail(&args.codex_args);
        let (dry_run_arg, codex_args) = extract_prodex_dry_run_flag(&codex_args);
        let (mut codex_args, include_code_review) =
            prepare_codex_launch_args(&codex_args, args.full_access);
        let mut model_provider_override =
            codex_cli_config_override_value(&codex_args, "model_provider");
        let profile_v2_name = codex_cli_profile_v2_name(&codex_args);
        let mut model_context_window_tokens =
            runtime_launch_cli_model_context_window_tokens(&codex_args);
        let mut gemini_thinking_budget_tokens =
            runtime_launch_cli_gemini_thinking_budget_tokens(&codex_args);
        let dry_run = args.dry_run || dry_run_arg;
        if !dry_run {
            repair_resume_session_metadata_prefix_from_codex_args(&codex_args)?;
        }
        let auto_external_provider = if model_provider_override.is_none() {
            runtime_resume_external_provider_from_codex_args(&codex_args)?
        } else {
            None
        };
        let auto_external_provider_base_url = auto_external_provider.map(|provider| {
            args.base_url
                .as_deref()
                .unwrap_or_else(|| provider.default_base_url())
                .to_string()
        });
        if let Some(provider) = auto_external_provider {
            let provider_args = super_external_provider_codex_args(
                provider,
                auto_external_provider_base_url
                    .as_deref()
                    .unwrap_or_else(|| provider.default_base_url()),
                None,
                None,
                None,
            );
            let mut next_args = Vec::with_capacity(provider_args.len() + codex_args.len());
            next_args.extend(provider_args);
            next_args.extend(codex_args);
            codex_args = next_args;
            model_provider_override = Some(provider.model_provider_id().to_string());
            model_context_window_tokens =
                runtime_launch_cli_model_context_window_tokens(&codex_args);
            gemini_thinking_budget_tokens =
                runtime_launch_cli_gemini_thinking_budget_tokens(&codex_args);
        }
        let delete_session_id = if dry_run {
            None
        } else {
            resolve_codex_delete_session_id(&codex_args)?
        };
        Ok(Self {
            args,
            codex_args,
            include_code_review,
            mem_mode,
            dry_run,
            model_provider_override,
            profile_v2_name,
            model_context_window_tokens,
            gemini_thinking_budget_tokens,
            auto_external_provider,
            auto_external_provider_base_url,
            delete_session_id,
        })
    }
}

fn resolve_codex_delete_session_id(codex_args: &[OsString]) -> Result<Option<String>> {
    let Some(selector) = codex_delete_session_selector(codex_args) else {
        return Ok(None);
    };
    if is_full_codex_session_id(selector) {
        return Ok(Some(selector.to_string()));
    }

    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let reports =
        prodex_session_store::collect_session_reports(&paths.shared_codex_root, None, &state)?;
    Ok(
        match prodex_session_store::resolve_session_report_by_id(&reports, selector) {
            Ok(report) => Some(report.id.clone()),
            Err(prodex_session_store::SessionResolveError::Missing { .. })
            | Err(prodex_session_store::SessionResolveError::Ambiguous { .. }) => None,
        },
    )
}

fn codex_delete_session_selector(codex_args: &[OsString]) -> Option<&str> {
    if codex_args.first().and_then(|arg| arg.to_str())? != "delete" {
        return None;
    }
    codex_args
        .iter()
        .skip(1)
        .rev()
        .filter_map(|arg| arg.to_str())
        .find(|arg| {
            let trimmed = arg.trim();
            !trimmed.is_empty() && !trimmed.starts_with('-')
        })
}

fn is_full_codex_session_id(selector: &str) -> bool {
    let bytes = selector.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

fn cleanup_codex_deleted_session_binding(session_id: Option<&str>) -> Result<()> {
    let Some(session_id) = session_id else {
        return Ok(());
    };
    let paths = AppPaths::discover()?;
    let _lock = acquire_state_file_lock(&paths)?;
    let mut state = AppState::load(&paths)?;
    let compact_key = prodex_runtime_store::runtime_compact_session_lineage_key(session_id);
    let removed_session = state.session_profile_bindings.remove(session_id).is_some();
    let removed_compact = state
        .session_profile_bindings
        .remove(&compact_key)
        .is_some();
    let removed = removed_session || removed_compact;
    if removed {
        let json =
            serde_json::to_string_pretty(&state).context("failed to serialize prodex state")?;
        write_state_json_atomic(&paths, &json)?;
    }
    Ok(())
}

fn runtime_resume_external_provider_from_codex_args(
    codex_args: &[OsString],
) -> Result<Option<SuperExternalProvider>> {
    let Some(session_id) = prodex_runtime_launch::codex_resume_session_id(codex_args) else {
        return Ok(None);
    };
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let reports =
        prodex_session_store::collect_session_reports(&paths.shared_codex_root, None, &state)?;
    let report = match prodex_session_store::resolve_session_report_by_id(&reports, session_id) {
        Ok(report) => report,
        Err(prodex_session_store::SessionResolveError::Missing { .. })
        | Err(prodex_session_store::SessionResolveError::Ambiguous { .. }) => return Ok(None),
    };
    Ok(report
        .model_provider
        .as_deref()
        .and_then(runtime_external_provider_from_model_provider_id))
}

fn runtime_external_provider_from_model_provider_id(
    model_provider: &str,
) -> Option<SuperExternalProvider> {
    let model_provider = model_provider.trim();
    if model_provider.eq_ignore_ascii_case(SUPER_GEMINI_PROVIDER_ID) {
        return Some(SuperExternalProvider::Gemini);
    }
    if model_provider.eq_ignore_ascii_case(SUPER_ANTHROPIC_PROVIDER_ID) {
        return Some(SuperExternalProvider::Anthropic);
    }
    if model_provider.eq_ignore_ascii_case(SUPER_COPILOT_PROVIDER_ID) {
        return Some(SuperExternalProvider::Copilot);
    }
    if model_provider.eq_ignore_ascii_case(SUPER_DEEPSEEK_PROVIDER_ID) {
        return Some(SuperExternalProvider::DeepSeek);
    }
    None
}

impl RuntimeLaunchStrategy for RunCommandStrategy {
    fn runtime_request(&self) -> RuntimeLaunchRequest<'_> {
        RuntimeLaunchRequest {
            profile: self.args.profile.as_deref(),
            allow_auto_rotate: !self.args.no_auto_rotate,
            skip_quota_check: self.args.skip_quota_check,
            base_url: self
                .args
                .base_url
                .as_deref()
                .or(self.auto_external_provider_base_url.as_deref()),
            upstream_no_proxy: self.args.no_proxy,
            include_code_review: self.include_code_review,
            smart_context_enabled: self.auto_external_provider.is_some(),
            presidio_redaction_enabled: false,
            model_context_window_tokens: self.model_context_window_tokens,
            gemini_thinking_budget_tokens: self.gemini_thinking_budget_tokens,
            force_runtime_proxy: false,
            model_provider_override: self.model_provider_override.as_deref(),
            profile_v2_name: self.profile_v2_name.as_deref(),
            external_provider: self
                .auto_external_provider
                .map(SuperExternalProvider::as_str),
            external_provider_api_key: None,
        }
    }

    fn build_plan(
        &self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan> {
        repair_resume_session_in_home(&prepared.codex_home, &self.codex_args)?;
        if let Some(mem_mode) = self.mem_mode {
            ensure_runtime_mem_prodex_observer(&prepared.paths)?;
            ensure_runtime_mem_codex_watch_for_home_with_mode(&prepared.codex_home, mem_mode)?;
        }
        let codex_args =
            profile_openai_compatible_codex_args(&prepared.codex_home, &self.codex_args);
        let codex_args = prepare_provider_capability_codex_args(&prepared.codex_home, &codex_args)?;
        let runtime_args = runtime_proxy_codex_passthrough_args(runtime_proxy, &codex_args);
        let mut child = codex_child_plan(prepared.codex_home.clone(), runtime_args);
        if self.args.no_proxy && runtime_proxy.is_none() {
            remove_upstream_proxy_env(&mut child);
        }
        Ok(RuntimeLaunchPlan::new(child))
    }

    fn after_child_exit(&self, status: &std::process::ExitStatus) -> Result<()> {
        if status.success() {
            cleanup_codex_deleted_session_binding(self.delete_session_id.as_deref())?;
        }
        Ok(())
    }
}

pub(super) fn handle_run(args: RunArgs) -> Result<()> {
    if run_launch_route(&args) == RunLaunchRoute::CodexCommandServerDirectPassthrough {
        return handle_codex_command_server_direct_passthrough(args);
    }

    let strategy = RunCommandStrategy::new(args)?;
    if strategy.dry_run {
        return print_runtime_launch_dry_run(
            "run",
            strategy.runtime_request(),
            RuntimeLaunchDryRunChild::Codex {
                codex_args: strategy.codex_args.clone(),
            },
            None,
        );
    }
    execute_runtime_launch(strategy)
}

pub(super) fn handle_gateway(args: GatewayArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let policy = prodex_runtime_policy::runtime_policy_gateway().unwrap_or_default();
    let provider = args
        .provider
        .or_else(|| policy.provider.as_deref().and_then(gateway_policy_provider));
    let provider_name = provider.map(SuperExternalProvider::as_str);
    let upstream_base_url = gateway_upstream_base_url(&args, &policy, provider)?;
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
    let gateway_admin_tokens = gateway_admin_tokens_config(auth_token.as_deref(), &policy)?;
    if policy.require_auth == Some(true) && auth_token.is_none() && policy.virtual_keys.is_empty() {
        bail!(
            "gateway auth is required by policy.toml; set --auth-token, PRODEX_GATEWAY_TOKEN, or [[gateway.virtual_keys]]; [[gateway.admin_tokens]] only protects admin endpoints"
        );
    }
    let gateway_virtual_keys = gateway_virtual_keys_config(&policy)?;
    let gateway_auth_required = auth_token.is_some() || !gateway_virtual_keys.is_empty();
    if policy.require_auth == Some(true) && !gateway_auth_required {
        bail!("gateway auth is required by policy.toml; configured virtual key env vars are empty");
    }
    if gateway_auth_required
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
    gateway_validate_listen_auth(&listen_addr, gateway_auth_required)?;
    let gateway_provider_id = gateway_provider_catalog_id(provider);
    let gateway_route_aliases = policy
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
                    gateway_provider_id,
                    &models,
                    &alias.model_metrics,
                ),
                models,
                strategy,
            }
        })
        .collect::<Vec<_>>();
    let gateway_guardrails = runtime_proxy_crate::RuntimeGatewayGuardrailConfig {
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
    };
    let presidio_redaction_enabled = if args.presidio {
        true
    } else if args.no_presidio {
        false
    } else {
        policy.guardrails.presidio_redaction.unwrap_or(false)
    };
    let gateway_call_id_header = policy
        .observability
        .call_id_header
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("x-prodex-call-id")
        .to_string();
    let gateway_state_store = gateway_state_store_config(&paths, &policy)?;
    let gateway_sso = gateway_sso_config(&policy)?;
    let gateway_observability = gateway_observability_config(&paths, &policy)?;
    let gateway_guardrail_webhook = gateway_guardrail_webhook_config(&policy);
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &state,
        upstream_base_url,
        provider: provider_options,
        upstream_no_proxy: false,
        smart_context_enabled: args.smart_context,
        presidio_redaction_enabled,
        model_context_window_tokens: None,
        preferred_listen_addr: Some(&listen_addr),
        gateway_auth_token_hash: auth_token
            .as_deref()
            .map(runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token),
        gateway_admin_tokens,
        gateway_sso,
        gateway_state_store,
        gateway_virtual_keys,
        gateway_route_aliases,
        gateway_guardrails,
        gateway_guardrail_webhook,
        gateway_call_id_header: Some(gateway_call_id_header),
        gateway_observability,
    })?;
    println!(
        "Prodex gateway listening on http://{} provider={} auth_required={} endpoints=/v1/responses,/v1/chat/completions,/v1/embeddings,/v1/images/*,/v1/audio/*,/v1/batches,/v1/rerank,/v1/a2a,/v1/messages models=/v1/models",
        proxy.listen_addr,
        provider_name.unwrap_or("openai-compatible"),
        gateway_auth_required
    );
    loop {
        std::thread::park();
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

fn gateway_state_store_config(
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

fn gateway_sso_config(
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

fn gateway_budget_usd_to_microusd(value: f64) -> u64 {
    (value * 1_000_000.0).round().clamp(1.0, u64::MAX as f64) as u64
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
                .map(|api_keys| RuntimeLocalRewriteProviderOptions::DeepSeek { api_keys })
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

fn gateway_api_keys_from_list(value: &str) -> Option<Vec<String>> {
    let keys = value
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!keys.is_empty()).then_some(keys)
}

fn gateway_validate_listen_auth(listen_addr: &str, auth_required: bool) -> Result<()> {
    let host = listen_addr
        .rsplit_once(':')
        .map(|(host, _)| host.trim_matches(['[', ']']))
        .unwrap_or(listen_addr)
        .trim();
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|addr| addr.is_loopback());
    if !loopback && !auth_required {
        bail!(
            "refusing to bind unauthenticated gateway on non-loopback address {listen_addr}; set --auth-token, PRODEX_GATEWAY_TOKEN, or [[gateway.virtual_keys]]"
        );
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunLaunchRoute {
    ManagedRuntime,
    CodexCommandServerDirectPassthrough,
}

fn run_launch_route(args: &RunArgs) -> RunLaunchRoute {
    // Command servers own stdio; keep Prodex preflight/proxy wrapping out of the stream.
    if !args.dry_run && is_codex_command_server_subcommand(&args.codex_args) {
        return RunLaunchRoute::CodexCommandServerDirectPassthrough;
    }
    RunLaunchRoute::ManagedRuntime
}

fn handle_codex_command_server_direct_passthrough(args: RunArgs) -> Result<()> {
    let plan = codex_command_server_direct_passthrough_plan(args)?;
    exit_with_status(run_child_plan(&plan, None)?)
}

fn codex_command_server_direct_passthrough_plan(args: RunArgs) -> Result<ChildProcessPlan> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let selection = RuntimeLaunchSelection::resolve(
        &paths,
        &state,
        args.profile.as_deref(),
        None,
        None,
        None,
        None,
    )?;

    if !selection.profileless_local_home
        && state
            .profiles
            .get(&selection.selected_profile_name)
            .with_context(|| format!("profile '{}' is missing", selection.selected_profile_name))?
            .managed
    {
        prepare_managed_codex_home(&paths, &selection.codex_home)?;
    }

    let codex_args = prodex_runtime_launch::normalize_codex_profile_args(&args.codex_args);
    repair_resume_session_in_home(&selection.codex_home, &codex_args)?;
    let mut child = codex_child_plan(selection.codex_home, codex_args);
    if args.no_proxy {
        remove_upstream_proxy_env(&mut child);
    }
    Ok(child)
}

#[derive(Debug, Clone)]
struct RuntimeLaunchSelection {
    initial_profile_name: String,
    selected_profile_name: String,
    codex_home: PathBuf,
    explicit_profile_requested: bool,
    non_openai_model_provider: Option<CodexModelProviderSetting>,
    profileless_local_home: bool,
}

impl RuntimeLaunchSelection {
    fn resolve(
        paths: &AppPaths,
        state: &AppState,
        requested: Option<&str>,
        model_provider_override: Option<&str>,
        profile_v2_name: Option<&str>,
        external_provider: Option<&str>,
        external_provider_api_key: Option<&str>,
    ) -> Result<Self> {
        let profileless_model_provider = codex_non_openai_model_provider_with_profile_v2(
            &paths.shared_codex_root,
            model_provider_override,
            profile_v2_name,
        );
        let gemini_external_provider = external_provider.is_some_and(|provider| {
            provider.eq_ignore_ascii_case("gemini") || provider.eq_ignore_ascii_case("gemini-oauth")
        });
        let copilot_external_provider = external_provider.is_some_and(|provider| {
            provider.eq_ignore_ascii_case("copilot")
                || provider.eq_ignore_ascii_case("github-copilot")
                || provider.eq_ignore_ascii_case("github_copilot")
        });
        let anthropic_external_provider = external_provider.is_some_and(|provider| {
            provider.eq_ignore_ascii_case("anthropic") || provider.eq_ignore_ascii_case("claude")
        });
        if requested.is_none()
            && (state.profiles.is_empty()
                || runtime_launch_should_use_profileless_gemini(
                    state,
                    gemini_external_provider,
                    external_provider_api_key,
                )
                || runtime_launch_should_use_profileless_external_provider(
                    state,
                    external_provider,
                    external_provider_api_key,
                ))
            && profileless_model_provider
                .as_ref()
                .is_some_and(runtime_launch_model_provider_uses_local_rewrite)
        {
            let codex_home = paths.shared_codex_root.clone();
            return Ok(Self {
                initial_profile_name: "local".to_string(),
                selected_profile_name: "local".to_string(),
                codex_home,
                explicit_profile_requested: false,
                non_openai_model_provider: profileless_model_provider,
                profileless_local_home: true,
            });
        }

        let profile_name = if gemini_external_provider {
            resolve_gemini_runtime_launch_profile_name(state, requested)?
        } else if copilot_external_provider {
            resolve_copilot_runtime_launch_profile_name(state, requested)?
        } else if anthropic_external_provider {
            resolve_anthropic_runtime_launch_profile_name(state, requested)?
        } else {
            resolve_runtime_launch_profile_name(state, requested)?
        };
        let codex_home = runtime_launch_profile_home_for_external_provider(
            state,
            &profile_name,
            external_provider,
        )?;
        let non_openai_model_provider = codex_non_openai_model_provider_with_profile_v2(
            &codex_home,
            model_provider_override,
            profile_v2_name,
        )
        .or_else(|| {
            profile_openai_compatible_model_provider_for_launch(
                &codex_home,
                model_provider_override,
            )
        });

        Ok(Self {
            initial_profile_name: profile_name.clone(),
            selected_profile_name: profile_name,
            codex_home,
            explicit_profile_requested: requested.is_some(),
            non_openai_model_provider,
            profileless_local_home: false,
        })
    }

    fn select_profile(
        &mut self,
        state: &AppState,
        profile_name: &str,
        model_provider_override: Option<&str>,
        profile_v2_name: Option<&str>,
    ) -> Result<()> {
        self.codex_home = runtime_launch_profile_home(state, profile_name)?;
        self.selected_profile_name = profile_name.to_string();
        self.non_openai_model_provider = codex_non_openai_model_provider_with_profile_v2(
            &self.codex_home,
            model_provider_override,
            profile_v2_name,
        )
        .or_else(|| {
            profile_openai_compatible_model_provider_for_launch(
                &self.codex_home,
                model_provider_override,
            )
        });
        Ok(())
    }
}

fn profile_openai_compatible_model_provider_for_launch(
    codex_home: &Path,
    model_provider_override: Option<&str>,
) -> Option<CodexModelProviderSetting> {
    if model_provider_override.is_some() {
        return None;
    }
    read_profile_openai_compatible_base_url(codex_home).map(|_| CodexModelProviderSetting {
        provider_id: PRODEX_OPENAI_COMPAT_PROVIDER_ID.to_string(),
        source: CodexModelProviderSource::CliOverride,
    })
}

pub(crate) fn resolve_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    let profile_name = resolve_profile_name(state, requested)?;
    if requested.is_some() {
        return Ok(profile_name);
    }

    if state
        .profiles
        .get(&profile_name)
        .is_some_and(|profile| profile.provider.supports_codex_runtime())
    {
        return Ok(profile_name);
    }

    active_profile_selection_order(state, &profile_name)
        .into_iter()
        .find(|candidate_name| {
            state.profiles.get(candidate_name).is_some_and(|profile| {
                profile.codex_home.exists() && profile.provider.supports_codex_runtime()
            })
        })
        .ok_or_else(|| {
            let (display_name, route_policy) = state
                .profiles
                .get(&profile_name)
                .map(|profile| {
                    (
                        profile.provider.display_name(),
                        profile.provider.capabilities().runtime_route_policy.label(),
                    )
                })
                .unwrap_or(("an unsupported provider", "unsupported"));
            anyhow::anyhow!(
                "profile '{}' uses {} (route {}). `prodex run` currently supports native OpenAI/Codex profiles only; provider adapters are launched through `prodex s --provider <provider>`.",
                profile_name,
                display_name,
                route_policy,
            )
        })
}

struct RuntimeLaunchPreparationBuilder<'a> {
    request: RuntimeLaunchRequest<'a>,
    paths: AppPaths,
    state: AppState,
    selection: RuntimeLaunchSelection,
}

impl<'a> RuntimeLaunchPreparationBuilder<'a> {
    fn from_request(request: RuntimeLaunchRequest<'a>) -> Result<Self> {
        let paths = AppPaths::discover()?;
        let mut state = AppState::load(&paths)?;
        let selection = select_runtime_launch_profile(&paths, &mut state, &request)?;

        Ok(Self {
            request,
            paths,
            state,
            selection,
        })
    }

    fn build(mut self) -> Result<PreparedRuntimeLaunch> {
        self.record_selection()?;
        self.handle_non_openai_model_provider()?;

        if self.selection.profileless_local_home {
            create_codex_home_if_missing(&self.selection.codex_home)?;
        }

        let managed = self.selected_profile_is_managed()?;
        if managed {
            prepare_managed_codex_home(&self.paths, &self.selection.codex_home)?;
        }

        let runtime_proxy = RuntimeProxyStartupFactory::build(
            &self.paths,
            &self.state,
            &self.selection,
            &self.request,
        )?;

        let RuntimeLaunchPreparationBuilder {
            paths, selection, ..
        } = self;
        Ok(PreparedRuntimeLaunch {
            paths,
            codex_home: selection.codex_home,
            managed,
            runtime_proxy,
        })
    }

    fn handle_non_openai_model_provider(&self) -> Result<()> {
        let Some(setting) = self.selection.non_openai_model_provider.as_ref() else {
            return Ok(());
        };

        if self.request.force_runtime_proxy {
            bail!(
                "profile '{}' uses model_provider '{}' from {}. `prodex claude` requires the default OpenAI/Codex provider.",
                self.selection.selected_profile_name,
                setting.provider_id,
                setting.source.display_name(),
            );
        }

        if local_rewrite_proxy_upstream_base_url(&self.selection, &self.request).is_some() {
            print_wrapped_stderr(&section_header("Runtime Provider"));
            if let Some(provider) = self.request.external_provider {
                let rotation = if runtime_external_provider_has_rotation_summary(provider) {
                    runtime_external_provider_rotation_summary(
                        &self.state,
                        &self.selection.selected_profile_name,
                        provider,
                        self.request.external_provider_api_key,
                        self.request.allow_auto_rotate,
                    )
                } else {
                    "Quota preflight and account rotation stay disabled.".to_string()
                };
                print_wrapped_stderr(&format!(
                    "Using provider '{provider}' through the Smart Context rewrite proxy. {rotation}",
                ));
            } else {
                print_wrapped_stderr(
                    "Using prodex-local through the Smart Context rewrite proxy. Quota preflight and account rotation stay disabled.",
                );
            }
            return Ok(());
        }

        print_wrapped_stderr(&section_header("Runtime Provider"));
        print_wrapped_stderr(&format_runtime_provider_direct_launch_message(
            setting.provider_id.as_str(),
            setting.source.display_name(),
        ));
        Ok(())
    }

    fn record_selection(&mut self) -> Result<()> {
        if self.selection.profileless_local_home {
            return Ok(());
        }

        record_run_selection(&mut self.state, &self.selection.selected_profile_name);
        self.state.save(&self.paths)?;
        Ok(())
    }

    fn selected_profile_is_managed(&self) -> Result<bool> {
        if self.selection.profileless_local_home {
            return Ok(false);
        }

        Ok(self
            .state
            .profiles
            .get(&self.selection.selected_profile_name)
            .with_context(|| {
                format!(
                    "profile '{}' is missing",
                    self.selection.selected_profile_name
                )
            })?
            .managed)
    }
}

struct RuntimeProxyStartupFactory;

impl RuntimeProxyStartupFactory {
    fn build(
        paths: &AppPaths,
        state: &AppState,
        selection: &RuntimeLaunchSelection,
        request: &RuntimeLaunchRequest<'_>,
    ) -> Result<Option<RuntimeProxyEndpoint>> {
        if let Some(local_upstream_base_url) =
            local_rewrite_proxy_upstream_base_url(selection, request)
        {
            return Ok(Some(start_local_rewrite_proxy_endpoint(
                paths,
                state,
                selection,
                request,
                local_upstream_base_url,
            )?));
        }

        if selection.non_openai_model_provider.is_some() {
            return Ok(None);
        }

        let runtime_upstream_base_url = quota_base_url(request.base_url);
        if request.presidio_redaction_enabled || request.smart_context_enabled {
            return Ok(Some(start_dedicated_runtime_proxy_endpoint(
                paths,
                state,
                selection,
                request,
                runtime_upstream_base_url,
            )?));
        }
        if request.force_runtime_proxy && !request.allow_auto_rotate {
            return Ok(Some(start_fixed_runtime_proxy_endpoint(
                paths,
                state,
                selection,
                request,
                runtime_upstream_base_url,
            )?));
        }
        if request.force_runtime_proxy
            || should_enable_runtime_rotation_proxy(
                state,
                &selection.selected_profile_name,
                request.allow_auto_rotate,
            )
        {
            return Ok(Some(ensure_runtime_rotation_proxy_endpoint(
                paths,
                &selection.selected_profile_name,
                runtime_upstream_base_url.as_str(),
                request.include_code_review,
                request.upstream_no_proxy,
                request.smart_context_enabled,
                runtime_launch_effective_model_context_window_tokens(request, selection),
            )?));
        }

        Ok(None)
    }

    fn preview(
        paths: &AppPaths,
        state: &AppState,
        selection: &RuntimeLaunchSelection,
        request: &RuntimeLaunchRequest<'_>,
    ) -> Result<Option<RuntimeProxyEndpoint>> {
        if local_rewrite_proxy_upstream_base_url(selection, request).is_some() {
            return Ok(Some(runtime_local_rewrite_proxy_dry_run_endpoint(
                paths, selection, request,
            )?));
        }

        if selection.non_openai_model_provider.is_some() {
            return Ok(None);
        }

        if request.presidio_redaction_enabled
            || request.force_runtime_proxy
            || request.smart_context_enabled
            || should_enable_runtime_rotation_proxy(
                state,
                &selection.selected_profile_name,
                request.allow_auto_rotate,
            )
        {
            return Ok(Some(runtime_proxy_dry_run_endpoint(paths)?));
        }

        Ok(None)
    }
}

pub(super) fn prepare_runtime_launch(
    request: RuntimeLaunchRequest<'_>,
) -> Result<PreparedRuntimeLaunch> {
    RuntimeLaunchPreparationBuilder::from_request(request)?.build()
}

pub(super) fn prepare_runtime_launch_dry_run(
    request: RuntimeLaunchRequest<'_>,
) -> Result<PreparedRuntimeLaunch> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let selection = RuntimeLaunchSelection::resolve(
        &paths,
        &state,
        request.profile,
        request.model_provider_override,
        request.profile_v2_name,
        request.external_provider,
        request.external_provider_api_key,
    )?;
    let managed = if selection.profileless_local_home {
        false
    } else {
        state
            .profiles
            .get(&selection.selected_profile_name)
            .with_context(|| format!("profile '{}' is missing", selection.selected_profile_name))?
            .managed
    };
    let runtime_proxy = RuntimeProxyStartupFactory::preview(&paths, &state, &selection, &request)?;

    Ok(PreparedRuntimeLaunch {
        paths,
        codex_home: selection.codex_home,
        managed,
        runtime_proxy,
    })
}

fn runtime_proxy_dry_run_endpoint(paths: &AppPaths) -> Result<RuntimeProxyEndpoint> {
    Ok(RuntimeProxyEndpoint {
        listen_addr: "127.0.0.1:0"
            .parse()
            .context("failed to build dry-run runtime proxy address")?,
        openai_mount_path: RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string(),
        local_model_provider_id: None,
        realtime_ws_base_url: None,
        realtime_ws_model: None,
        lease_dir: paths.root.join("runtime-broker-dry-run-leases"),
        _lease: None,
        _direct_proxy: None,
    })
}

fn runtime_local_rewrite_proxy_dry_run_endpoint(
    paths: &AppPaths,
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
) -> Result<RuntimeProxyEndpoint> {
    let local_model_provider_id = runtime_local_rewrite_model_provider_id(selection, request)
        .unwrap_or(SUPER_LOCAL_PROVIDER_ID);
    Ok(RuntimeProxyEndpoint {
        listen_addr: "127.0.0.1:0"
            .parse()
            .context("failed to build dry-run runtime local rewrite proxy address")?,
        openai_mount_path: RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH.to_string(),
        local_model_provider_id: Some(local_model_provider_id.to_string()),
        realtime_ws_base_url: None,
        realtime_ws_model: None,
        lease_dir: paths.root.join("runtime-local-proxy-dry-run-leases"),
        _lease: None,
        _direct_proxy: None,
    })
}

fn start_dedicated_runtime_proxy_endpoint(
    paths: &AppPaths,
    state: &AppState,
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
    runtime_upstream_base_url: String,
) -> Result<RuntimeProxyEndpoint> {
    let model_context_window_tokens =
        runtime_launch_effective_model_context_window_tokens(request, selection);
    let proxy = start_runtime_rotation_proxy_with_options(RuntimeRotationProxyStartOptions {
        paths,
        state,
        current_profile: &selection.selected_profile_name,
        upstream_base_url: runtime_upstream_base_url,
        include_code_review: request.include_code_review,
        upstream_no_proxy: request.upstream_no_proxy,
        smart_context_enabled: request.smart_context_enabled,
        presidio_redaction_enabled: request.presidio_redaction_enabled,
        model_context_window_tokens,
        preferred_listen_addr: None,
    })?;
    Ok(RuntimeProxyEndpoint {
        listen_addr: proxy.listen_addr,
        openai_mount_path: RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string(),
        local_model_provider_id: None,
        realtime_ws_base_url: None,
        realtime_ws_model: None,
        lease_dir: paths.root.join("runtime-dedicated-proxy-leases"),
        _lease: None,
        _direct_proxy: Some(proxy),
    })
}

fn start_fixed_runtime_proxy_endpoint(
    paths: &AppPaths,
    state: &AppState,
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
    runtime_upstream_base_url: String,
) -> Result<RuntimeProxyEndpoint> {
    let model_context_window_tokens =
        runtime_launch_effective_model_context_window_tokens(request, selection);
    let fixed_state = fixed_runtime_proxy_state(state, &selection.selected_profile_name)?;
    let proxy = start_runtime_rotation_proxy_with_options(RuntimeRotationProxyStartOptions {
        paths,
        state: &fixed_state,
        current_profile: &selection.selected_profile_name,
        upstream_base_url: runtime_upstream_base_url,
        include_code_review: request.include_code_review,
        upstream_no_proxy: request.upstream_no_proxy,
        smart_context_enabled: request.smart_context_enabled,
        presidio_redaction_enabled: request.presidio_redaction_enabled,
        model_context_window_tokens,
        preferred_listen_addr: None,
    })?;
    Ok(RuntimeProxyEndpoint {
        listen_addr: proxy.listen_addr,
        openai_mount_path: RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string(),
        local_model_provider_id: None,
        realtime_ws_base_url: None,
        realtime_ws_model: None,
        lease_dir: paths.root.join("runtime-fixed-proxy-leases"),
        _lease: None,
        _direct_proxy: Some(proxy),
    })
}

fn start_local_rewrite_proxy_endpoint(
    paths: &AppPaths,
    state: &AppState,
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
    upstream_base_url: String,
) -> Result<RuntimeProxyEndpoint> {
    let model_context_window_tokens =
        runtime_launch_effective_model_context_window_tokens(request, selection);
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths,
        state,
        upstream_base_url,
        provider: runtime_local_rewrite_provider_options(state, selection, request)?,
        upstream_no_proxy: request.upstream_no_proxy,
        smart_context_enabled: request.smart_context_enabled,
        presidio_redaction_enabled: request.presidio_redaction_enabled,
        model_context_window_tokens,
        preferred_listen_addr: None,
        gateway_auth_token_hash: None,
        gateway_admin_tokens: Vec::new(),
        gateway_sso: RuntimeGatewaySsoConfig::default(),
        gateway_state_store: RuntimeGatewayStateStore::file(paths),
        gateway_virtual_keys: Vec::new(),
        gateway_route_aliases: Vec::new(),
        gateway_guardrails: runtime_proxy_crate::RuntimeGatewayGuardrailConfig::default(),
        gateway_guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
        gateway_call_id_header: None,
        gateway_observability: RuntimeGatewayObservabilityConfig::default(),
    })?;
    let local_model_provider_id = runtime_local_rewrite_model_provider_id(selection, request)
        .unwrap_or(SUPER_LOCAL_PROVIDER_ID);
    Ok(RuntimeProxyEndpoint {
        listen_addr: proxy.listen_addr,
        openai_mount_path: RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH.to_string(),
        local_model_provider_id: Some(local_model_provider_id.to_string()),
        realtime_ws_base_url: proxy
            .gemini_live_sidecar_addr
            .map(|addr| format!("http://{addr}")),
        realtime_ws_model: proxy.gemini_live_sidecar_model.clone(),
        lease_dir: paths.root.join("runtime-local-proxy-leases"),
        _lease: None,
        _direct_proxy: Some(proxy),
    })
}

fn local_rewrite_proxy_upstream_base_url(
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
) -> Option<String> {
    if request.force_runtime_proxy || !request.smart_context_enabled {
        return None;
    }
    let provider = selection.non_openai_model_provider.as_ref()?;
    if !runtime_launch_model_provider_uses_local_rewrite(provider) {
        return None;
    }
    request
        .base_url
        .map(str::to_string)
        .or_else(|| {
            codex_config_value_with_profile_v2(
                &selection.codex_home,
                &format!("model_providers.{}.base_url", provider.provider_id.as_str()),
                request.profile_v2_name,
            )
        })
        .filter(|base_url| !base_url.trim().is_empty())
}

fn fixed_runtime_proxy_state(state: &AppState, profile_name: &str) -> Result<AppState> {
    prodex_runtime_launch::fixed_runtime_proxy_state(state, profile_name)
}

fn runtime_launch_effective_model_context_window_tokens(
    request: &RuntimeLaunchRequest<'_>,
    selection: &RuntimeLaunchSelection,
) -> Option<u64> {
    if !request.smart_context_enabled {
        return None;
    }
    request.model_context_window_tokens.or_else(|| {
        runtime_launch_config_model_context_window_tokens_with_profile_v2(
            &selection.codex_home,
            request.profile_v2_name,
        )
    })
}

fn runtime_launch_effective_gemini_thinking_budget_tokens(
    request: &RuntimeLaunchRequest<'_>,
    selection: &RuntimeLaunchSelection,
) -> Option<u64> {
    request.gemini_thinking_budget_tokens.or_else(|| {
        runtime_launch_config_gemini_thinking_budget_tokens_with_profile_v2(
            &selection.codex_home,
            request.profile_v2_name,
        )
    })
}

#[cfg(test)]
#[path = "../../tests/src/app_commands/runtime_launch.rs"]
mod tests;
