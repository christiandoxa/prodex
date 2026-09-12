use super::{
    GoalResumeRelaunchPlan, GoalUsageLimitMonitor, PreparedRuntimeLaunch, RunArgs,
    RuntimeLaunchPlan, RuntimeLaunchRequest, RuntimeLaunchStrategy, RuntimeProxyEndpoint,
    SuperExternalProvider, add_runtime_goal_session_tracking,
    cleanup_codex_deleted_session_binding, codex_child_plan, codex_cli_config_override_value,
    codex_cli_profile_v2_name, codex_tui_child_plan, extract_prodex_dry_run_flag,
    is_codex_command_server_subcommand, isolate_auto_external_provider_child_env,
    maintain_shared_codex_sessions_after_child_exit, prepare_codex_launch_args,
    prepare_goal_usage_limit_monitor, prepare_provider_capability_codex_args,
    profile_openai_compatible_codex_args, remove_first_codex_config_override_pair,
    remove_upstream_proxy_env, repair_resume_session_in_home,
    repair_resume_session_metadata_prefix_from_codex_args, resolve_codex_delete_session_id,
    restore_resume_session_settings, runtime_exit_status_is_cancelled,
    runtime_launch_cli_gemini_thinking_budget_tokens, runtime_launch_cli_model,
    runtime_launch_cli_model_context_window_tokens, runtime_launch_openai_model_context_codex_args,
    runtime_proxy_codex_passthrough_args, runtime_resume_external_provider_from_codex_args,
    runtime_resume_session_settings_from_codex_args, runtime_session_recovery_wait_message,
    super_external_provider_codex_args, wait_for_runtime_recovery_round,
};
use anyhow::Result;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::PathBuf;

pub(super) struct RunCommandStrategy {
    pub(super) args: RunArgs,
    pub(super) codex_args: Vec<OsString>,
    pub(super) command_server: bool,
    pub(super) include_code_review: bool,
    pub(super) dry_run: bool,
    pub(super) model_provider_override: Option<String>,
    pub(super) profile_v2_name: Option<String>,
    pub(super) model_context_window_tokens: Option<u64>,
    pub(super) gemini_thinking_budget_tokens: Option<u64>,
    pub(super) auto_external_provider: Option<SuperExternalProvider>,
    pub(super) auto_external_provider_base_url: Option<String>,
    pub(super) delete_session_id: Option<String>,
    pub(super) auto_goal_resume_attempted_profiles: BTreeSet<String>,
    pub(super) goal_usage_limit_monitor: Option<GoalUsageLimitMonitor>,
    pub(super) pending_goal_resume_plan: Option<GoalResumeRelaunchPlan>,
    pub(super) goal_resume_session_affinity_release: Option<String>,
    pub(super) recovery_model: Option<String>,
    pub(super) runtime_recovery_log_target: Option<crate::RuntimeRecoveryLogTarget>,
    pub(super) transient_recovery_rounds: usize,
    pub(super) recovery_generation: usize,
    pub(super) allow_failed_profile_recovery: bool,
    pub(super) model_preference_sync: Option<crate::ModelPreferenceSync>,
    pub(super) resume_session_path: Option<PathBuf>,
}

impl RunCommandStrategy {
    pub(super) fn new(args: RunArgs) -> Result<Self> {
        let codex_feature_args = args.codex_args_with_feature_overrides();
        let (dry_run_arg, codex_args) = extract_prodex_dry_run_flag(&codex_feature_args);
        let (mut codex_args, include_code_review) =
            prepare_codex_launch_args(&codex_args, args.full_access);
        let command_server = is_codex_command_server_subcommand(&codex_args);
        let mut model_provider_override =
            codex_cli_config_override_value(&codex_args, "model_provider");
        let profile_v2_name = codex_cli_profile_v2_name(&codex_args);
        let mut model_context_window_tokens =
            runtime_launch_cli_model_context_window_tokens(&codex_args);
        let mut gemini_thinking_budget_tokens =
            runtime_launch_cli_gemini_thinking_budget_tokens(&codex_args);
        let dry_run = args.dry_run || dry_run_arg;
        let resume_session_path = (!dry_run)
            .then(|| repair_resume_session_metadata_prefix_from_codex_args(&codex_args))
            .transpose()?
            .flatten();
        let session_settings = runtime_resume_session_settings_from_codex_args(&codex_args);
        let model_is_explicit = runtime_launch_cli_model(&codex_args).is_some()
            || codex_cli_config_override_value(&codex_args, "model").is_some();
        let effort_is_explicit =
            codex_cli_config_override_value(&codex_args, "model_reasoning_effort").is_some();
        restore_resume_session_settings(
            &mut codex_args,
            session_settings.as_ref(),
            model_is_explicit,
            effort_is_explicit,
        );
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
            let mut provider_args = super_external_provider_codex_args(
                provider,
                auto_external_provider_base_url
                    .as_deref()
                    .unwrap_or_else(|| provider.default_base_url()),
                None,
                None,
                None,
            );
            remove_first_codex_config_override_pair(&mut provider_args, "model");
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
            prepare_goal_usage_limit_monitor(&codex_args, dry_run || args.no_auto_rotate)?;
        Ok(Self {
            args,
            codex_args,
            command_server,
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
            recovery_model: None,
            runtime_recovery_log_target: None,
            transient_recovery_rounds: 0,
            recovery_generation: 0,
            allow_failed_profile_recovery: false,
            model_preference_sync: None,
            resume_session_path,
        })
    }

    fn project_in_app_resume_settings(
        &self,
        prepared: &PreparedRuntimeLaunch,
        codex_args: &mut Vec<OsString>,
        preference_context: &crate::ModelPreferenceContext,
    ) -> Result<()> {
        if self.command_server
            || prodex_runtime_launch::is_codex_exec_invocation(codex_args)
            || prodex_runtime_launch::codex_resume_requested(codex_args)
        {
            return Ok(());
        }
        crate::project_in_app_resume_model_settings(
            if prepared.managed {
                &prepared.paths.shared_codex_root
            } else {
                &prepared.codex_home
            },
            codex_args,
            self.profile_v2_name.as_deref(),
            [
                ("model", preference_context.explicit_model.is_none()),
                (
                    "model_provider",
                    !prepared.managed && self.model_provider_override.is_none(),
                ),
                (
                    "model_reasoning_effort",
                    preference_context.explicit_effort.is_none(),
                ),
            ],
        )
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
        &mut self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan> {
        if let Some(path) = repair_resume_session_in_home(&prepared.codex_home, &self.codex_args)?
            && path.starts_with(&prepared.paths.shared_codex_root)
        {
            self.resume_session_path = Some(path);
        }
        let codex_args =
            runtime_launch_openai_model_context_codex_args(&prepared.codex_home, &self.codex_args)?;
        let codex_args = profile_openai_compatible_codex_args(&prepared.codex_home, &codex_args)?;
        let preference_context = crate::resolve_fresh_model_preference_context(
            &prepared.paths,
            &prepared.codex_home,
            &codex_args,
        )?;
        let codex_args = crate::apply_fresh_model_preference_selection(
            &prepared.codex_home,
            codex_args,
            &preference_context,
            true,
            false,
        );
        let codex_args = prepare_provider_capability_codex_args(&prepared.codex_home, &codex_args)?;
        let mut codex_args = crate::apply_fresh_model_preference_selection(
            &prepared.codex_home,
            codex_args,
            &preference_context,
            false,
            true,
        );
        self.recovery_model =
            crate::codex_effective_config_value(&prepared.codex_home, &codex_args, "model")?;
        self.runtime_recovery_log_target =
            runtime_proxy.and_then(RuntimeProxyEndpoint::recovery_log_target);
        self.project_in_app_resume_settings(prepared, &mut codex_args, &preference_context)?;
        if let Some(monitor) = self.goal_usage_limit_monitor.as_ref() {
            add_runtime_goal_session_tracking(
                &prepared.codex_home,
                self.profile_v2_name.as_deref(),
                &mut codex_args,
                &monitor.marker_path,
            )?;
        }
        let runtime_args = runtime_proxy_codex_passthrough_args(runtime_proxy, &codex_args);
        let mut child = if self.command_server {
            codex_child_plan(prepared.codex_home.clone(), runtime_args)
        } else {
            codex_tui_child_plan(prepared.codex_home.clone(), runtime_args)
        };
        isolate_auto_external_provider_child_env(self.auto_external_provider, &mut child);
        if self.args.no_proxy && runtime_proxy.is_none() {
            remove_upstream_proxy_env(&mut child);
        }
        if prepared.managed
            && !child
                .extra_env
                .iter()
                .any(|(key, _)| key == "CODEX_SQLITE_HOME")
        {
            child.extra_env.push((
                "CODEX_SQLITE_HOME".into(),
                prepared.paths.shared_codex_root.as_os_str().to_os_string(),
            ));
        }
        if !self.dry_run && !self.command_server {
            crate::runtime_thread_index::repair_dirty_thread_index(&prepared.paths, &child);
        }
        if !self.dry_run && !self.command_server {
            self.model_preference_sync = match crate::ModelPreferenceSync::start_with_scope(
                &prepared.paths,
                &child,
                preference_context.logical_scope.clone(),
            ) {
                Ok(sync) => Some(sync),
                Err(_error) => {
                    crate::print_launch_status(
                        "model preference synchronization unavailable; continuing",
                    );
                    None
                }
            };
        }
        Ok(RuntimeLaunchPlan::new(child))
    }

    fn child_exit_requested(&mut self) -> Result<bool> {
        if self.pending_goal_resume_plan.is_some() {
            return Ok(true);
        }
        let (session_id, failure_class, retry_pool, observed_model, evidence) =
            match self.goal_usage_limit_monitor.as_mut() {
                Some(monitor) => (
                    monitor.take_usage_limit_signal()?,
                    monitor.recovery_failure_class(),
                    monitor.recovery_retries_after_pool_round(),
                    monitor.recovery_model().map(ToOwned::to_owned),
                    monitor.recovery_evidence(),
                ),
                None => return Ok(false),
            };
        if observed_model.is_some() {
            self.recovery_model = observed_model;
        }
        let Some(session_id) = session_id else {
            return Ok(false);
        };
        let plan = self.plan_live_goal_resume_relaunch(&session_id)?;
        if plan.is_none() && retry_pool {
            if !self.auto_goal_resume_attempted_profiles.is_empty() {
                self.transient_recovery_rounds = self.transient_recovery_rounds.saturating_add(1);
                self.auto_goal_resume_attempted_profiles.clear();
            }
            if let Some(target) = self.runtime_recovery_log_target.as_ref() {
                target.log(&runtime_session_recovery_wait_message(
                    self.transient_recovery_rounds,
                    failure_class,
                ));
            }
            self.allow_failed_profile_recovery = true;
            return Ok(false);
        }
        let Some(mut plan) = plan else {
            return Ok(false);
        };
        plan.failure_class = failure_class;
        plan.evidence = evidence;
        self.pending_goal_resume_plan = Some(plan);
        Ok(true)
    }

    fn monitors_child_exit(&self) -> bool {
        self.goal_usage_limit_monitor.is_some()
    }

    fn session_affinity_release(&self) -> Option<&str> {
        self.goal_resume_session_affinity_release.as_deref()
    }

    fn relaunch_after_child_exit(&mut self, status: &std::process::ExitStatus) -> Result<bool> {
        if runtime_exit_status_is_cancelled(status) {
            self.pending_goal_resume_plan = None;
            return Ok(false);
        }
        let plan = match self.pending_goal_resume_plan.take() {
            Some(plan) => Some(plan),
            None => {
                let mut plan = self.plan_goal_resume_relaunch(status)?;
                let retry_pool = self
                    .goal_usage_limit_monitor
                    .as_ref()
                    .is_some_and(GoalUsageLimitMonitor::recovery_retries_after_pool_round);
                if plan.is_none() && retry_pool {
                    let failure_class = self
                        .goal_usage_limit_monitor
                        .as_ref()
                        .map(GoalUsageLimitMonitor::recovery_failure_class)
                        .unwrap_or("transport");
                    if let Some(target) = self.runtime_recovery_log_target.as_ref() {
                        target.log(&runtime_session_recovery_wait_message(
                            self.transient_recovery_rounds.saturating_add(1),
                            failure_class,
                        ));
                    }
                    if !wait_for_runtime_recovery_round() {
                        return Ok(false);
                    }
                    self.transient_recovery_rounds =
                        self.transient_recovery_rounds.saturating_add(1);
                    self.auto_goal_resume_attempted_profiles.clear();
                    self.allow_failed_profile_recovery = true;
                    plan = self.next_observed_goal_resume_relaunch()?;
                }
                plan
            }
        };
        let Some(plan) = plan else {
            return Ok(false);
        };
        if let Some(model) = self
            .goal_usage_limit_monitor
            .as_ref()
            .and_then(GoalUsageLimitMonitor::recovery_model)
        {
            self.recovery_model = Some(model.to_string());
        }
        self.apply_goal_resume_relaunch(plan)?;
        Ok(true)
    }

    fn after_child_exit(
        &mut self,
        status: &std::process::ExitStatus,
        plan: &RuntimeLaunchPlan,
    ) -> Result<()> {
        if let Some(sync) = self.model_preference_sync.as_mut()
            && let Some(_error) = sync.finish()
        {
            crate::print_launch_status("model preference synchronization was incomplete");
        }
        if let Some(session_file) = self.resume_session_path.as_deref() {
            super::maintain_shared_codex_session_after_child_exit(&plan.child, session_file);
        } else {
            maintain_shared_codex_sessions_after_child_exit(&plan.child);
        }
        if status.success() {
            cleanup_codex_deleted_session_binding(self.delete_session_id.as_deref())?;
        }
        Ok(())
    }
}
