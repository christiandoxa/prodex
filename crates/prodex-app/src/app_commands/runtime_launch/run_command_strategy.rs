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
    restore_resume_session_settings, runtime_launch_cli_gemini_thinking_budget_tokens,
    runtime_launch_cli_model, runtime_launch_cli_model_context_window_tokens,
    runtime_launch_openai_spark_context_codex_args, runtime_proxy_codex_passthrough_args,
    runtime_resume_external_provider_from_codex_args,
    runtime_resume_session_settings_from_codex_args, super_external_provider_codex_args,
};
use anyhow::Result;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;

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
    pub(super) model_preference_sync: Option<crate::ModelPreferenceSync>,
}

fn apply_remembered_model_preference(
    codex_home: &Path,
    codex_args: Vec<OsString>,
    selection: Option<&crate::LastModelSelection>,
) -> Vec<OsString> {
    let Some(selection) = selection else {
        return codex_args;
    };
    if crate::model_preference_model_is_compatible(codex_home, &codex_args, selection) {
        crate::apply_model_preference_selection(codex_home, codex_args, selection, false, true)
    } else {
        let mut codex_args = codex_args;
        crate::remove_remembered_model_override(&mut codex_args);
        codex_args
    }
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
        if !dry_run {
            repair_resume_session_metadata_prefix_from_codex_args(&codex_args)?;
        }
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
            model_preference_sync: None,
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
        &mut self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan> {
        repair_resume_session_in_home(&prepared.codex_home, &self.codex_args)?;
        let codex_args =
            runtime_launch_openai_spark_context_codex_args(&prepared.codex_home, &self.codex_args)?;
        let codex_args = profile_openai_compatible_codex_args(&prepared.codex_home, &codex_args)?;
        let remembered_selection = crate::resolve_fresh_model_preference(
            &prepared.paths,
            &prepared.codex_home,
            &codex_args,
        )?;
        let codex_args = remembered_selection
            .as_ref()
            .map(|selection| {
                crate::apply_model_preference_selection(
                    &prepared.codex_home,
                    codex_args.clone(),
                    selection,
                    true,
                    false,
                )
            })
            .unwrap_or(codex_args);
        let codex_args = prepare_provider_capability_codex_args(&prepared.codex_home, &codex_args)?;
        let mut codex_args = apply_remembered_model_preference(
            &prepared.codex_home,
            codex_args,
            remembered_selection.as_ref(),
        );
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
            self.model_preference_sync =
                match crate::ModelPreferenceSync::start(&prepared.paths, &child) {
                    Ok(sync) => Some(sync),
                    Err(_error) => {
                        crate::print_launch_status(
                            "model preference synchronization unavailable; continuing",
                        );
                        None
                    }
                };
            if crate::reconcile_codex_thread_index(&child.binary, &child).is_err() {
                crate::print_launch_status("session index reconciliation unavailable; continuing");
            }
        }
        Ok(RuntimeLaunchPlan::new(child))
    }

    fn child_exit_requested(&mut self) -> Result<bool> {
        let session_id = match self.goal_usage_limit_monitor.as_mut() {
            Some(monitor) => monitor.take_usage_limit_signal()?,
            None => return Ok(false),
        };
        let Some(session_id) = session_id else {
            return Ok(false);
        };
        let Some(plan) = self.plan_live_goal_resume_relaunch(&session_id)? else {
            return Ok(false);
        };
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
        if !self.command_server
            && crate::reconcile_codex_thread_index(&plan.child.binary, &plan.child).is_err()
        {
            crate::print_launch_status(
                "session index reconciliation unavailable after exit; continuing",
            );
        }
        maintain_shared_codex_sessions_after_child_exit();
        if status.success() {
            cleanup_codex_deleted_session_binding(self.delete_session_id.as_deref())?;
        }
        Ok(())
    }
}
