use super::*;
mod command_server;
pub(crate) mod gateway_config;
#[path = "runtime_launch/gateway_shutdown.rs"]
mod gateway_shutdown;
#[path = "runtime_launch/gateway_startup.rs"]
pub(crate) mod gateway_startup;
mod gateway_status;
pub(crate) mod goal_resume;
mod preflight;
mod provider_names;
mod providers;
mod resume_provider;
pub(crate) mod resume_repair;
mod run_command_strategy;
mod selection;
mod session_delete;
pub(super) use command_server::codex_app_server_broker_launch;
#[cfg(test)]
use command_server::prepare_codex_command_server_runtime_launch;
use command_server::{
    RunLaunchRoute, execute_codex_command_server_managed_runtime, run_launch_route,
};
#[cfg(test)]
use gateway_config::{
    gateway_admin_tokens_config, gateway_call_id_header_config, gateway_guardrail_config,
    gateway_guardrail_webhook_config, gateway_observability_config, gateway_openai_api_keys,
    gateway_route_alias_model_metrics, gateway_route_aliases_config, gateway_sso_config,
    gateway_state_store_config, gateway_upstream_base_url, gateway_virtual_keys_config,
    resolve_gateway_auth_config, resolve_gateway_guardrail_config,
};
#[cfg(test)]
use gateway_config::{resolve_gateway_launch_config, resolve_gateway_launch_config_with_secrets};
use gateway_startup::start_gateway_backend;
use gateway_status::print_gateway_status;
use goal_resume::*;
pub(super) use resume_provider::runtime_resume_provider_from_codex_args;
use resume_provider::{
    RuntimeResumeSessionSettings, runtime_resume_external_provider_from_codex_args,
    runtime_resume_session_settings_from_codex_args,
};
use run_command_strategy::RunCommandStrategy;
use selection::RuntimeLaunchSelection;
pub(crate) use selection::resolve_runtime_launch_profile_name;
use session_delete::{
    cleanup_codex_deleted_session_binding, clear_codex_session_binding,
    maintain_shared_codex_sessions_after_child_exit, resolve_codex_delete_session_id,
};
use std::borrow::Cow;
use std::path::Path;
use {preflight::*, provider_names::*, providers::*, resume_repair::*};

pub(crate) fn handle_run(args: RunArgs) -> Result<()> {
    if let Some(base_url) = args.base_url.as_deref() {
        validate_credential_free_http_url(base_url, "runtime upstream base URL")?;
    }
    let route = run_launch_route(&args);
    let strategy = RunCommandStrategy::new(args)?;
    if strategy.dry_run {
        return print_runtime_launch_dry_run(
            "run",
            strategy.runtime_request(),
            RuntimeLaunchDryRunChild::Codex {
                codex_args: strategy.codex_args.clone(),
            },
            None,
            None,
        );
    }
    match route {
        RunLaunchRoute::ManagedRuntime => execute_runtime_launch(strategy),
        RunLaunchRoute::CodexCommandServerManagedStdio => {
            execute_codex_command_server_managed_runtime(strategy)
        }
    }
}

pub(crate) fn resolved_super_runtime_tool_args(args: SuperArgs, presidio: bool) -> RuntimeToolArgs {
    let normalized = prodex_runtime_launch::normalize_run_codex_args(&args.codex_args);
    let is_resume = prodex_runtime_launch::codex_resume_session_id(&normalized).is_some();
    let model_is_explicit = args.local_model.is_some()
        || codex_cli_config_override_value(&args.codex_args, "model").is_some();
    let effort_is_explicit =
        codex_cli_config_override_value(&args.codex_args, "model_reasoning_effort").is_some();
    let session_settings = is_resume
        .then(|| runtime_resume_session_settings_from_codex_args(&normalized))
        .flatten();
    let mut runtime_args = args.into_runtime_tool_args_with_presidio(presidio);
    if is_resume && !model_is_explicit {
        remove_first_codex_config_override_pair(&mut runtime_args.codex_args, "model");
    }
    restore_resume_session_settings(
        &mut runtime_args.codex_args,
        session_settings.as_ref(),
        model_is_explicit,
        effort_is_explicit,
    );
    runtime_args
}

pub(crate) fn resolve_super_dry_run_main_agent(args: &mut SuperArgs) -> Result<()> {
    let Some(session_provider) = runtime_resume_provider_from_codex_args(&args.codex_args)? else {
        return Ok(());
    };
    super::resolve_super_main_agent_with_prompt(args, false, Some(session_provider), |_, _| {
        bail!("Super provider prompt is unavailable during dry-run")
    })
    .map(|_| ())
}

fn restore_resume_session_settings(
    codex_args: &mut Vec<OsString>,
    settings: Option<&RuntimeResumeSessionSettings>,
    model_is_explicit: bool,
    effort_is_explicit: bool,
) {
    let Some(settings) = settings else {
        return;
    };
    for (key, value, is_explicit) in [
        ("model", settings.model.as_deref(), model_is_explicit),
        (
            "model_reasoning_effort",
            settings.reasoning_effort.as_deref(),
            effort_is_explicit,
        ),
    ] {
        if is_explicit {
            continue;
        }
        let Some(value) = value else {
            continue;
        };
        codex_args.splice(
            0..0,
            [
                OsString::from("-c"),
                OsString::from(format!(
                    "{key}={}",
                    crate::runtime_catalog_config::toml_string_literal(value)
                )),
            ],
        );
    }
}

pub(super) fn remove_first_codex_config_override_pair(args: &mut Vec<OsString>, key: &str) -> bool {
    let mut index = 0;
    while index + 1 < args.len() {
        let is_config_flag = matches!(args[index].to_str(), Some("-c" | "--config"));
        let matches_key = args[index + 1]
            .to_str()
            .and_then(|assignment| assignment.split_once('='))
            .is_some_and(|(candidate, _)| candidate.trim() == key);
        if is_config_flag && matches_key {
            args.drain(index..=index + 1);
            return true;
        }
        index += 1;
    }
    false
}

pub(super) fn handle_gateway(args: GatewayArgs) -> Result<()> {
    let backend = start_gateway_backend(args)?;
    print_gateway_status(
        backend.listen_addr(),
        backend.provider_name(),
        backend.auth_required(),
    )?;
    gateway_shutdown::wait_for_signal_and_drain(&backend)
}

struct RuntimeLaunchPreparationBuilder<'a> {
    request: RuntimeLaunchRequest<'a>,
    resolved_harness: prodex_provider_core::ResolvedHarnessMode,
    paths: AppPaths,
    state: AppState,
    selection: RuntimeLaunchSelection,
}

impl<'a> RuntimeLaunchPreparationBuilder<'a> {
    fn from_request(
        request: RuntimeLaunchRequest<'a>,
        resolved_harness: prodex_provider_core::ResolvedHarnessMode,
    ) -> Result<Self> {
        let paths = AppPaths::discover()?;
        let mut state = AppState::load_and_repair(&paths)?;
        let selection = select_runtime_launch_profile(&paths, &mut state, &request)?;

        Ok(Self {
            request,
            resolved_harness,
            paths,
            state,
            selection,
        })
    }

    fn build(self) -> Result<PreparedRuntimeLaunch> {
        self.build_with_terminal_output(true)
    }

    fn build_with_terminal_output(
        mut self,
        terminal_output: bool,
    ) -> Result<PreparedRuntimeLaunch> {
        self.record_selection()?;
        if terminal_output {
            self.handle_non_openai_model_provider()?;
        }

        if self.selection.profileless_local_home {
            create_codex_home_if_missing(&self.selection.codex_home)?;
        }

        let managed = self.selected_profile_is_managed()?;
        if managed {
            ensure_managed_runtime_launch_home_under_root(&self.paths, &self.selection.codex_home)?;
            // ponytail: only targeted resume repair belongs here; full maintenance runs after exit.
            prepare_managed_codex_home_for_runtime_launch(&self.paths, &self.selection.codex_home)?;
        }

        let runtime_proxy = RuntimeProxyStartupFactory::build(
            &self.paths,
            &self.state,
            &self.selection,
            &self.request,
            self.resolved_harness,
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

        if local_rewrite_proxy_upstream_base_url(&self.selection, &self.request)?.is_some() {
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
                print_stderr_panel(
                    "Runtime Provider",
                    &[format!(
                        "Using provider '{provider}' through the local compatibility proxy. Smart Context rewrites require a proven tokenizer. {rotation}",
                    )],
                )?;
            } else {
                print_stderr_panel(
                    "Runtime Provider",
                    &["Using prodex-local through the local compatibility proxy. Smart Context rewrites require a proven tokenizer. Quota preflight and account rotation stay disabled.".to_string()],
                )?;
            }
            return Ok(());
        }

        print_stderr_panel(
            "Runtime Provider",
            &[format_runtime_provider_direct_launch_message(
                setting.provider_id.as_str(),
                setting.source.display_name(),
            )],
        )?;
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

fn ensure_managed_runtime_launch_home_under_root(
    paths: &AppPaths,
    codex_home: &Path,
) -> Result<()> {
    prodex_shared_codex_fs::ensure_managed_profiles_root(paths)?;
    if !prodex_core::path_is_strictly_under_root(&paths.managed_profiles_root, codex_home) {
        bail!(
            "managed profile home {} is outside {}",
            codex_home.display(),
            paths.managed_profiles_root.display()
        );
    }
    Ok(())
}

struct RuntimeProxyStartupFactory;

impl RuntimeProxyStartupFactory {
    fn build(
        paths: &AppPaths,
        state: &AppState,
        selection: &RuntimeLaunchSelection,
        request: &RuntimeLaunchRequest<'_>,
        resolved_harness: prodex_provider_core::ResolvedHarnessMode,
    ) -> Result<Option<RuntimeProxyEndpoint>> {
        if request
            .external_provider
            .is_some_and(|provider| provider.eq_ignore_ascii_case("gemini-native"))
        {
            return Ok(None);
        }
        if runtime_launch_uses_kiro_connect_proxy(request) {
            let proxy = start_runtime_kiro_connect_proxy(paths, request.upstream_no_proxy)?;
            return Ok(Some(RuntimeProxyEndpoint {
                listen_addr: proxy.listen_addr(),
                openai_mount_path: String::new(),
                local_model_provider_id: None,
                force_http_responses: false,
                realtime_ws_base_url: None,
                realtime_ws_model: None,
                lease_dir: paths.root.join("runtime-kiro-connect-proxy-leases"),
                broker_session_affinity_control: None,
                _lease: None,
                _direct_proxy: None,
                _kiro_connect_proxy: Some(proxy),
            }));
        }

        if let Some(local_upstream_base_url) =
            local_rewrite_proxy_upstream_base_url(selection, request)?
        {
            return Ok(Some(start_local_rewrite_proxy_endpoint(
                paths,
                state,
                selection,
                request,
                local_upstream_base_url,
                resolved_harness,
            )?));
        }

        if selection.non_openai_model_provider.is_some() {
            return Ok(None);
        }

        let runtime_upstream_base_url = quota_base_url(request.base_url)?;
        let response_governance_enabled =
            RuntimeConfig::from_env_policy_and_cli(paths)?.force_http_response_transport();
        if request.presidio_redaction_enabled || request.smart_context_enabled {
            return Ok(Some(start_runtime_proxy_endpoint(
                paths,
                state,
                selection,
                request,
                runtime_upstream_base_url,
                !request.allow_auto_rotate,
            )?));
        }
        if (request.force_runtime_proxy || response_governance_enabled)
            && !request.allow_auto_rotate
        {
            return Ok(Some(start_runtime_proxy_endpoint(
                paths,
                state,
                selection,
                request,
                runtime_upstream_base_url,
                true,
            )?));
        }
        if request.force_runtime_proxy
            || response_governance_enabled
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
                runtime_launch_effective_model_context_window_tokens(
                    request,
                    &selection.codex_home,
                )?,
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
        if request
            .external_provider
            .is_some_and(|provider| provider.eq_ignore_ascii_case("gemini-native"))
        {
            return Ok(None);
        }
        if runtime_launch_uses_kiro_connect_proxy(request) {
            let proxy = RuntimeKiroConnectProxy::dry_run();
            return Ok(Some(RuntimeProxyEndpoint {
                listen_addr: proxy.listen_addr(),
                openai_mount_path: String::new(),
                local_model_provider_id: None,
                force_http_responses: false,
                realtime_ws_base_url: None,
                realtime_ws_model: None,
                lease_dir: paths.root.join("runtime-kiro-connect-proxy-dry-run-leases"),
                broker_session_affinity_control: None,
                _lease: None,
                _direct_proxy: None,
                _kiro_connect_proxy: Some(proxy),
            }));
        }

        if local_rewrite_proxy_upstream_base_url(selection, request)?.is_some() {
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
            || RuntimeConfig::from_env_policy_and_cli(paths)?.force_http_response_transport()
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

#[cfg(test)]
pub(super) fn prepare_runtime_launch(
    request: RuntimeLaunchRequest<'_>,
) -> Result<PreparedRuntimeLaunch> {
    prepare_runtime_launch_with_harness(
        request,
        prodex_provider_core::resolve_harness_mode(None, None),
    )
}

pub(crate) fn prepare_runtime_launch_with_harness(
    request: RuntimeLaunchRequest<'_>,
    resolved_harness: prodex_provider_core::ResolvedHarnessMode,
) -> Result<PreparedRuntimeLaunch> {
    RuntimeLaunchPreparationBuilder::from_request(request, resolved_harness)?.build()
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
    validate_runtime_launch_upstream_base_url(&selection, &request)?;
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
        force_http_responses: RuntimeConfig::from_env_policy_and_cli(paths)?
            .force_http_response_transport(),
        realtime_ws_base_url: None,
        realtime_ws_model: None,
        lease_dir: paths.root.join("runtime-broker-dry-run-leases"),
        broker_session_affinity_control: None,
        _lease: None,
        _direct_proxy: None,
        _kiro_connect_proxy: None,
    })
}

fn validate_runtime_launch_upstream_base_url(
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
) -> Result<()> {
    if let Some(base_url) = request.base_url {
        validate_credential_free_http_url(base_url, "runtime upstream base URL")?;
    }
    if selection.non_openai_model_provider.is_none() {
        quota_base_url(request.base_url)?;
    } else if request.base_url.is_none()
        && let Some(provider) = selection.non_openai_model_provider.as_ref()
        && let Some(base_url) = codex_config_value_with_profile_v2(
            &selection.codex_home,
            &format!("model_providers.{}.base_url", provider.provider_id),
            request.profile_v2_name,
        )?
    {
        validate_credential_free_http_url(&base_url, "runtime upstream base URL")?;
    }
    Ok(())
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
        force_http_responses: false,
        realtime_ws_base_url: None,
        realtime_ws_model: None,
        lease_dir: paths.root.join("runtime-local-proxy-dry-run-leases"),
        broker_session_affinity_control: None,
        _lease: None,
        _direct_proxy: None,
        _kiro_connect_proxy: None,
    })
}

fn start_runtime_proxy_endpoint(
    paths: &AppPaths,
    state: &AppState,
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
    runtime_upstream_base_url: String,
    fixed: bool,
) -> Result<RuntimeProxyEndpoint> {
    let model_context_window_tokens =
        runtime_launch_effective_model_context_window_tokens(request, &selection.codex_home)?;
    let proxy_state = runtime_proxy_endpoint_state(state, selection, fixed)?;
    let proxy = start_runtime_rotation_proxy_with_options(RuntimeRotationProxyStartOptions {
        paths,
        state: proxy_state.as_ref(),
        current_profile: &selection.selected_profile_name,
        upstream_base_url: runtime_upstream_base_url,
        include_code_review: request.include_code_review,
        upstream_no_proxy: request.upstream_no_proxy,
        auto_redeem: request.auto_redeem,
        smart_context_enabled: request.smart_context_enabled,
        presidio_redaction_enabled: request.presidio_redaction_enabled,
        model_context_window_tokens,
        preferred_listen_addr: None,
    })?;
    Ok(RuntimeProxyEndpoint {
        listen_addr: proxy.listen_addr,
        openai_mount_path: RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string(),
        local_model_provider_id: None,
        force_http_responses: RuntimeConfig::from_env_policy_and_cli(paths)?
            .force_http_response_transport(),
        realtime_ws_base_url: proxy
            .realtime_ws_sidecar_addr
            .map(|addr| format!("http://{addr}{RUNTIME_PROXY_OPENAI_MOUNT_PATH}/realtime")),
        realtime_ws_model: None,
        lease_dir: paths.root.join(if fixed {
            "runtime-fixed-proxy-leases"
        } else {
            "runtime-dedicated-proxy-leases"
        }),
        broker_session_affinity_control: None,
        _lease: None,
        _direct_proxy: Some(proxy),
        _kiro_connect_proxy: None,
    })
}

fn start_local_rewrite_proxy_endpoint(
    paths: &AppPaths,
    state: &AppState,
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
    upstream_base_url: String,
    resolved_harness: prodex_provider_core::ResolvedHarnessMode,
) -> Result<RuntimeProxyEndpoint> {
    let model_context_window_tokens =
        runtime_launch_effective_model_context_window_tokens(request, &selection.codex_home)?;
    let proxy = start_runtime_local_rewrite_proxy_with_harness(
        RuntimeLocalRewriteProxyStartOptions {
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
        },
        resolved_harness,
    )?;
    let local_model_provider_id = runtime_local_rewrite_model_provider_id(selection, request)
        .unwrap_or(SUPER_LOCAL_PROVIDER_ID);
    Ok(RuntimeProxyEndpoint {
        listen_addr: proxy.listen_addr,
        openai_mount_path: RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH.to_string(),
        local_model_provider_id: Some(local_model_provider_id.to_string()),
        force_http_responses: false,
        realtime_ws_base_url: proxy
            .realtime_ws_sidecar_addr
            .map(|addr| format!("http://{addr}")),
        realtime_ws_model: proxy.realtime_ws_model.clone(),
        lease_dir: paths.root.join("runtime-local-proxy-leases"),
        broker_session_affinity_control: None,
        _lease: None,
        _direct_proxy: Some(proxy),
        _kiro_connect_proxy: None,
    })
}

fn local_rewrite_proxy_upstream_base_url(
    selection: &RuntimeLaunchSelection,
    request: &RuntimeLaunchRequest<'_>,
) -> Result<Option<String>> {
    if request.force_runtime_proxy || !request.smart_context_enabled {
        return Ok(None);
    }
    let Some(provider) = selection.non_openai_model_provider.as_ref() else {
        return Ok(None);
    };
    if !runtime_launch_model_provider_uses_local_rewrite(provider) {
        return Ok(None);
    }
    let base_url = match request.base_url {
        Some(base_url) => Some(base_url.to_string()),
        None => codex_config_value_with_profile_v2(
            &selection.codex_home,
            &format!("model_providers.{}.base_url", provider.provider_id.as_str()),
            request.profile_v2_name,
        )?,
    };
    if let Some(base_url) = base_url.as_deref() {
        validate_credential_free_http_url(base_url, "runtime upstream base URL")?;
    }
    Ok(base_url)
}

fn runtime_proxy_endpoint_state<'a>(
    state: &'a AppState,
    selection: &RuntimeLaunchSelection,
    fixed: bool,
) -> Result<Cow<'a, AppState>> {
    if fixed {
        fixed_runtime_proxy_state(state, &selection.selected_profile_name).map(Cow::Owned)
    } else {
        Ok(Cow::Borrowed(state))
    }
}

fn fixed_runtime_proxy_state(state: &AppState, profile_name: &str) -> Result<AppState> {
    prodex_runtime_launch::fixed_runtime_proxy_state(state, profile_name)
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
