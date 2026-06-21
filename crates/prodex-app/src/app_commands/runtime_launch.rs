use super::*;
mod gateway_config;
mod mem0_gateway;
mod preflight;
mod provider_names;
mod providers;
mod resume_provider;
mod resume_repair;
mod selection;
mod session_delete;
use gateway_config::resolve_gateway_launch_config;
#[cfg(test)]
use gateway_config::{gateway_sso_config, gateway_state_store_config};
pub(crate) use mem0_gateway::start_mem0_memory_gateway_for_runtime_request;
use resume_provider::runtime_resume_external_provider_from_codex_args;
use selection::RuntimeLaunchSelection;
pub(crate) use selection::resolve_runtime_launch_profile_name;
use session_delete::{cleanup_codex_deleted_session_binding, resolve_codex_delete_session_id};
use {preflight::*, provider_names::*, providers::*, resume_repair::*};

struct RunCommandStrategy {
    args: RunArgs,
    codex_args: Vec<OsString>,
    include_code_review: bool,
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
        let (dry_run_arg, codex_args) = extract_prodex_dry_run_flag(&args.codex_args);
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
    let gateway = resolve_gateway_launch_config(&paths, &args, &policy)?;
    let proxy = start_runtime_local_rewrite_proxy(RuntimeLocalRewriteProxyStartOptions {
        paths: &paths,
        state: &state,
        upstream_base_url: gateway.upstream_base_url,
        provider: gateway.provider_options,
        upstream_no_proxy: false,
        smart_context_enabled: args.smart_context,
        presidio_redaction_enabled: gateway.presidio_redaction_enabled,
        model_context_window_tokens: None,
        preferred_listen_addr: Some(&gateway.listen_addr),
        gateway_auth_token_hash: gateway.auth_token_hash,
        gateway_admin_tokens: gateway.admin_tokens,
        gateway_sso: gateway.sso,
        gateway_state_store: gateway.state_store,
        gateway_virtual_keys: gateway.virtual_keys,
        gateway_route_aliases: gateway.route_aliases,
        gateway_guardrails: gateway.guardrails,
        gateway_guardrail_webhook: gateway.guardrail_webhook,
        gateway_call_id_header: Some(gateway.call_id_header),
        gateway_observability: gateway.observability,
    })?;
    println!(
        "Prodex gateway listening on http://{} provider={} auth_required={} endpoints=/v1/responses,/v1/chat/completions,/v1/embeddings,/v1/images/*,/v1/audio/*,/v1/batches,/v1/rerank,/v1/a2a,/v1/messages models=/v1/models",
        proxy.listen_addr,
        gateway.provider_name.unwrap_or("openai-compatible"),
        gateway.auth_required
    );
    loop {
        std::thread::park();
    }
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
    let state = AppState::load_and_repair(&paths)?;
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

struct RuntimeLaunchPreparationBuilder<'a> {
    request: RuntimeLaunchRequest<'a>,
    paths: AppPaths,
    state: AppState,
    selection: RuntimeLaunchSelection,
}

impl<'a> RuntimeLaunchPreparationBuilder<'a> {
    fn from_request(request: RuntimeLaunchRequest<'a>) -> Result<Self> {
        let paths = AppPaths::discover()?;
        let mut state = AppState::load_and_repair(&paths)?;
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
