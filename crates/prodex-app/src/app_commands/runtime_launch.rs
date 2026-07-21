use super::*;
pub(crate) mod gateway_config;
#[path = "runtime_launch/gateway_shutdown.rs"]
mod gateway_shutdown;
#[path = "runtime_launch/gateway_startup.rs"]
mod gateway_startup;
mod gateway_status;
pub(crate) mod goal_resume;
mod preflight;
mod provider_names;
mod providers;
mod resume_provider;
mod resume_repair;
mod selection;
mod session_delete;
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
pub(crate) use gateway_startup::start_policy_gateway_backend as start_policy_gateway_backend_inner;
use gateway_status::print_gateway_status;
use goal_resume::*;
use resume_provider::runtime_resume_external_provider_from_codex_args;
use selection::RuntimeLaunchSelection;
pub(crate) use selection::resolve_runtime_launch_profile_name;
use session_delete::{
    cleanup_codex_deleted_session_binding, clear_codex_session_binding,
    maintain_shared_codex_sessions_after_child_exit, resolve_codex_delete_session_id,
};
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::path::Path;
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
    auto_goal_resume_attempted_profiles: BTreeSet<String>,
    goal_usage_limit_monitor: Option<GoalUsageLimitMonitor>,
    pending_goal_resume_plan: Option<GoalResumeRelaunchPlan>,
    goal_resume_session_affinity_release: Option<String>,
}

impl RunCommandStrategy {
    fn new(args: RunArgs) -> Result<Self> {
        let codex_feature_args = args.codex_args_with_feature_overrides();
        let (dry_run_arg, codex_args) = extract_prodex_dry_run_flag(&codex_feature_args);
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
        let goal_usage_limit_monitor =
            prepare_goal_usage_limit_monitor(&codex_args, dry_run || args.no_auto_rotate);
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
            auto_goal_resume_attempted_profiles: BTreeSet::new(),
            goal_usage_limit_monitor,
            pending_goal_resume_plan: None,
            goal_resume_session_affinity_release: None,
        })
    }
}

impl RuntimeLaunchStrategy for RunCommandStrategy {
    fn runtime_request(&self) -> RuntimeLaunchRequest<'_> {
        RuntimeLaunchRequest {
            profile: self.args.profile.as_deref(),
            allow_auto_rotate: !self.args.no_auto_rotate,
            auto_redeem: self.args.auto_redeem,
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
            runtime_launch_openai_spark_context_codex_args(&prepared.codex_home, &self.codex_args)?;
        let codex_args = profile_openai_compatible_codex_args(&prepared.codex_home, &codex_args)?;
        let mut codex_args =
            prepare_provider_capability_codex_args(&prepared.codex_home, &codex_args)?;
        if let Some(monitor) = self.goal_usage_limit_monitor.as_ref() {
            add_runtime_goal_session_tracking(
                &prepared.codex_home,
                self.profile_v2_name.as_deref(),
                &mut codex_args,
                &monitor.marker_path,
            );
        }
        let runtime_args = runtime_proxy_codex_passthrough_args(runtime_proxy, &codex_args);
        let mut child = codex_tui_child_plan(prepared.codex_home.clone(), runtime_args);
        isolate_auto_external_provider_child_env(self.auto_external_provider, &mut child);
        if self.args.no_proxy && runtime_proxy.is_none() {
            remove_upstream_proxy_env(&mut child);
        }
        Ok(RuntimeLaunchPlan::new(child))
    }

    fn child_exit_requested(&mut self) -> bool {
        let Some(session_id) = self
            .goal_usage_limit_monitor
            .as_mut()
            .and_then(GoalUsageLimitMonitor::take_usage_limit_signal)
        else {
            return false;
        };
        let Some(plan) = self
            .plan_live_goal_resume_relaunch(&session_id)
            .ok()
            .flatten()
        else {
            return false;
        };
        self.pending_goal_resume_plan = Some(plan);
        true
    }

    fn monitors_child_exit(&self) -> bool {
        self.goal_usage_limit_monitor.is_some()
    }

    fn session_affinity_release(&self) -> Option<&str> {
        self.goal_resume_session_affinity_release.as_deref()
    }

    fn relaunch_after_child_exit(&mut self, status: &std::process::ExitStatus) -> Result<bool> {
        let plan = match self.pending_goal_resume_plan.take() {
            Some(plan) => Some(plan),
            None => self.plan_goal_resume_relaunch(status)?,
        };
        let Some(plan) = plan else {
            return Ok(false);
        };
        self.apply_goal_resume_relaunch(plan)?;
        Ok(true)
    }

    fn after_child_exit(&mut self, status: &std::process::ExitStatus) -> Result<()> {
        maintain_shared_codex_sessions_after_child_exit();
        if status.success() {
            cleanup_codex_deleted_session_binding(self.delete_session_id.as_deref())?;
        }
        Ok(())
    }
}

pub(super) fn handle_run(args: RunArgs) -> Result<()> {
    if let Some(base_url) = args.base_url.as_deref() {
        validate_credential_free_http_url(base_url, "runtime upstream base URL")?;
    }
    if run_launch_route(&args) == RunLaunchRoute::CodexCommandServerDirectPassthrough {
        quota_base_url(args.base_url.as_deref())?;
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
            None,
        );
    }
    execute_runtime_launch(strategy)
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

pub(crate) fn start_policy_gateway_application_inner(
    service_mode: RuntimePolicyServiceMode,
    preferred_listen_addr: Option<String>,
) -> Result<GatewayApplication> {
    gateway_startup::start_policy_gateway_application(service_mode, preferred_listen_addr)
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
    codex_command_server_child_plan(args.profile.as_deref(), args.codex_args, args.no_proxy)
}

pub(super) fn codex_app_server_broker_child_plan(
    profile: Option<&str>,
) -> Result<ChildProcessPlan> {
    codex_command_server_child_plan(profile, vec![OsString::from("app-server")], false)
}

fn codex_command_server_child_plan(
    profile: Option<&str>,
    codex_args: Vec<OsString>,
    no_proxy: bool,
) -> Result<ChildProcessPlan> {
    let paths = AppPaths::discover()?;
    let state = AppState::load_and_repair(&paths)?;
    let selection =
        RuntimeLaunchSelection::resolve(&paths, &state, profile, None, None, None, None)?;

    if !selection.profileless_local_home
        && state
            .profiles
            .get(&selection.selected_profile_name)
            .with_context(|| format!("profile '{}' is missing", selection.selected_profile_name))?
            .managed
    {
        ensure_managed_runtime_launch_home_under_root(&paths, &selection.codex_home)?;
        prepare_managed_codex_home_for_runtime_launch(&paths, &selection.codex_home)?;
    }

    let codex_args = prodex_runtime_launch::normalize_codex_profile_args(&codex_args);
    repair_resume_session_in_home(&selection.codex_home, &codex_args)?;
    let mut child = codex_child_plan(selection.codex_home, codex_args);
    if no_proxy {
        remove_upstream_proxy_env(&mut child);
    }
    Ok(child)
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

    fn build(mut self) -> Result<PreparedRuntimeLaunch> {
        self.record_selection()?;
        self.handle_non_openai_model_provider()?;

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
                        "Using provider '{provider}' through the Smart Context rewrite proxy. {rotation}",
                    )],
                )?;
            } else {
                print_stderr_panel(
                    "Runtime Provider",
                    &["Using prodex-local through the Smart Context rewrite proxy. Quota preflight and account rotation stay disabled.".to_string()],
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
        if request.external_provider == Some("kiro") {
            let proxy = start_runtime_kiro_connect_proxy(paths, request.upstream_no_proxy)?;
            return Ok(Some(RuntimeProxyEndpoint {
                listen_addr: proxy.listen_addr(),
                openai_mount_path: String::new(),
                local_model_provider_id: None,
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
        if request.force_runtime_proxy && !request.allow_auto_rotate {
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
        if request.external_provider == Some("kiro") {
            let proxy = RuntimeKiroConnectProxy::dry_run();
            return Ok(Some(RuntimeProxyEndpoint {
                listen_addr: proxy.listen_addr(),
                openai_mount_path: String::new(),
                local_model_provider_id: None,
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

pub(super) fn prepare_runtime_launch_with_harness(
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
