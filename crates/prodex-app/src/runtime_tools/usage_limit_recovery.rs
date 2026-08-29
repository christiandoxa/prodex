use super::RuntimeToolLaunchStrategy;
use crate::app_commands::runtime_launch::{
    GoalResumeRelaunchPlan, RuntimeUsageLimitResumeOptions, next_runtime_usage_limit_plan,
    plan_runtime_usage_limit_relaunch,
};
use crate::app_state::AppStateIoExt;
use crate::{AppState, codex_cli_config_override_value, runtime_launch_cli_model};
use anyhow::Result;
use std::ffi::OsString;

const RUNTIME_USAGE_LIMIT_CONTINUATION_PROMPT: &str = "Continue the interrupted task from the persisted session. Preserve completed work and do not repeat completed tool calls.";

impl RuntimeToolLaunchStrategy {
    pub(super) fn observe_child_exit_request(&mut self) -> Result<bool> {
        let (session_id, paths) = {
            let Some(monitor) = self.goal_usage_limit_monitor.as_mut() else {
                return Ok(false);
            };
            (monitor.take_usage_limit_signal()?, monitor.paths.clone())
        };
        let Some(session_id) = session_id else {
            return Ok(false);
        };
        let state = AppState::load_and_repair(&paths)?;
        let options = RuntimeUsageLimitResumeOptions {
            requested_profile: self.args.profile.as_deref(),
            no_auto_rotate: self.args.no_auto_rotate,
            skip_quota_check: self.args.skip_quota_check,
            base_url: self.args.base_url.as_deref(),
            include_code_review: self.include_code_review,
            no_proxy: self.args.no_proxy,
            attempted_profiles: &self.auto_goal_resume_attempted_profiles,
        };
        let Some(plan) = next_runtime_usage_limit_plan(&state, &session_id, &options) else {
            return Ok(false);
        };
        self.pending_goal_resume_plan = Some(plan);
        Ok(true)
    }

    fn apply_goal_resume_relaunch(&mut self, plan: GoalResumeRelaunchPlan) -> Result<()> {
        let session_settings =
            crate::app_commands::runtime_launch::runtime_resume_session_settings_from_codex_args(
                &self.codex_args,
            );
        let model_is_explicit = runtime_launch_cli_model(&self.codex_args).is_some()
            || codex_cli_config_override_value(&self.codex_args, "model").is_some();
        let effort_is_explicit =
            codex_cli_config_override_value(&self.codex_args, "model_reasoning_effort").is_some();
        let exec_mode = prodex_runtime_launch::is_codex_exec_invocation(&self.codex_args);
        crate::app_commands::runtime_launch::clear_codex_session_binding(&plan.session_id)?;
        self.goal_resume_session_affinity_release = Some(plan.session_id.clone());
        self.codex_args = if exec_mode {
            prodex_runtime_launch::retarget_codex_exec_resume_args(
                &self.codex_args,
                &plan.session_id,
            )
        } else {
            prodex_runtime_launch::retarget_codex_tui_resume_args(
                &self.codex_args,
                &plan.session_id,
            )
        };
        let session_settings = session_settings.or_else(|| {
            crate::app_commands::runtime_launch::runtime_resume_session_settings_from_codex_args(
                &self.codex_args,
            )
        });
        crate::app_commands::runtime_launch::restore_resume_session_settings(
            &mut self.codex_args,
            session_settings.as_ref(),
            model_is_explicit,
            effort_is_explicit,
        );
        self.auto_goal_resume_attempted_profiles
            .insert(plan.profile_name.clone());
        self.args.profile = Some(plan.profile_name);
        if let Some(monitor) = self.goal_usage_limit_monitor.as_mut() {
            monitor.prepare_for_resume();
        }
        self.codex_args.push(if exec_mode {
            OsString::from(RUNTIME_USAGE_LIMIT_CONTINUATION_PROMPT)
        } else {
            OsString::from("/goal resume")
        });
        Ok(())
    }

    pub(super) fn relaunch_after_usage_limit(
        &mut self,
        status: &std::process::ExitStatus,
    ) -> Result<bool> {
        if self.args.no_auto_rotate {
            self.pending_goal_resume_plan = None;
            return Ok(false);
        }
        let plan = match self.pending_goal_resume_plan.take() {
            Some(plan) => Some(plan),
            None => {
                let options = RuntimeUsageLimitResumeOptions {
                    requested_profile: self.args.profile.as_deref(),
                    no_auto_rotate: self.args.no_auto_rotate,
                    skip_quota_check: self.args.skip_quota_check,
                    base_url: self.args.base_url.as_deref(),
                    include_code_review: self.include_code_review,
                    no_proxy: self.args.no_proxy,
                    attempted_profiles: &self.auto_goal_resume_attempted_profiles,
                };
                match self.goal_usage_limit_monitor.as_mut() {
                    Some(monitor) => plan_runtime_usage_limit_relaunch(monitor, status, &options)?,
                    None => None,
                }
            }
        };
        let Some(plan) = plan else {
            return Ok(false);
        };
        self.apply_goal_resume_relaunch(plan)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestEnvVarGuard;
    use crate::{Commands, parse_cli_command_from};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn super_exec_recovery_resumes_the_session_without_replaying_the_prompt() {
        let root = std::env::temp_dir().join(format!(
            "prodex-runtime-tool-recovery-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let _home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
        let shared = root.join("shared-codex-home");
        let _shared = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", shared.to_str().unwrap());
        let command = parse_cli_command_from([
            "prodex",
            "s",
            "--no-presidio",
            "--no-sub-agent",
            "exec",
            "original prompt",
        ])
        .unwrap();
        let Commands::Super(args) = command else {
            panic!("expected Super command");
        };
        let mut strategy =
            RuntimeToolLaunchStrategy::new(args.into_runtime_tool_args_with_presidio(false));
        strategy
            .apply_goal_resume_relaunch(GoalResumeRelaunchPlan {
                session_id: "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9".to_string(),
                profile_name: "profile-b".to_string(),
            })
            .unwrap();

        assert!(prodex_runtime_launch::is_codex_exec_invocation(
            &strategy.codex_args
        ));
        assert_eq!(
            prodex_runtime_launch::codex_resume_session_id(&strategy.codex_args),
            Some("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9")
        );
        assert!(
            !strategy
                .codex_args
                .iter()
                .any(|arg| arg == "original prompt")
        );
        assert_eq!(
            strategy.codex_args.last().and_then(|arg| arg.to_str()),
            Some(RUNTIME_USAGE_LIMIT_CONTINUATION_PROMPT)
        );
        assert_eq!(strategy.args.profile.as_deref(), Some("profile-b"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn super_tui_recovery_resumes_the_session_without_replaying_the_prompt() {
        let root = std::env::temp_dir().join(format!(
            "prodex-runtime-tool-tui-recovery-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let _home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
        let shared = root.join("shared-codex-home");
        let _shared = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", shared.to_str().unwrap());
        let command = parse_cli_command_from([
            "prodex",
            "s",
            "--no-presidio",
            "--no-sub-agent",
            "resume",
            "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
            "original prompt",
        ])
        .unwrap();
        let Commands::Super(args) = command else {
            panic!("expected Super command");
        };
        let mut strategy =
            RuntimeToolLaunchStrategy::new(args.into_runtime_tool_args_with_presidio(false));
        strategy
            .apply_goal_resume_relaunch(GoalResumeRelaunchPlan {
                session_id: "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9".to_string(),
                profile_name: "profile-b".to_string(),
            })
            .unwrap();

        assert_eq!(
            prodex_runtime_launch::codex_resume_session_id(&strategy.codex_args),
            Some("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9")
        );
        assert!(
            !strategy
                .codex_args
                .iter()
                .any(|arg| arg == "original prompt")
        );
        assert_eq!(
            strategy.codex_args.last().and_then(|arg| arg.to_str()),
            Some("/goal resume")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn no_auto_rotate_drops_pending_runtime_recovery() {
        let command = parse_cli_command_from([
            "prodex",
            "s",
            "--no-auto-rotate",
            "--no-presidio",
            "--no-sub-agent",
            "exec",
            "work",
        ])
        .unwrap();
        let Commands::Super(args) = command else {
            panic!("expected Super command");
        };
        let mut strategy =
            RuntimeToolLaunchStrategy::new(args.into_runtime_tool_args_with_presidio(false));
        strategy.pending_goal_resume_plan = Some(GoalResumeRelaunchPlan {
            session_id: "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9".to_string(),
            profile_name: "profile-b".to_string(),
        });

        assert!(
            !strategy
                .relaunch_after_usage_limit(&exit_status(1))
                .unwrap()
        );
        assert!(strategy.pending_goal_resume_plan.is_none());
    }

    fn exit_status(code: i32) -> std::process::ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt as _;
            std::process::ExitStatus::from_raw(code << 8)
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt as _;
            std::process::ExitStatus::from_raw(code as u32)
        }
    }
}
