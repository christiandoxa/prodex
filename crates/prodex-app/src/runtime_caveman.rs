use super::*;
#[allow(unused_imports)]
pub(crate) use prodex_caveman_assets::PRODEX_CAVEMAN_FULL_ASSETS_ENV;

pub(crate) struct CavemanLaunchStrategy {
    args: CavemanArgs,
    codex_args: Vec<OsString>,
    include_code_review: bool,
    mem_mode: Option<RuntimeMemTranscriptMode>,
    model_provider_override: Option<String>,
    model_context_window_tokens: Option<u64>,
}

impl CavemanLaunchStrategy {
    pub(crate) fn new(args: CavemanArgs) -> Self {
        let (mem_mode, codex_args) = runtime_mem_extract_mode_with_detail(&args.codex_args);
        let mem_mode = if args.smart_context {
            runtime_mem_super_default_transcript_mode(mem_mode)
        } else {
            mem_mode
        };
        let (codex_args, include_code_review) =
            prepare_codex_launch_args(&codex_args, args.full_access);
        let model_provider_override =
            codex_cli_config_override_value(&codex_args, "model_provider");
        let model_context_window_tokens =
            runtime_launch_cli_model_context_window_tokens(&codex_args);
        Self {
            args,
            codex_args,
            include_code_review,
            mem_mode,
            model_provider_override,
            model_context_window_tokens,
        }
    }
}

impl RuntimeLaunchStrategy for CavemanLaunchStrategy {
    fn runtime_request(&self) -> RuntimeLaunchRequest<'_> {
        RuntimeLaunchRequest {
            profile: self.args.profile.as_deref(),
            allow_auto_rotate: !self.args.no_auto_rotate,
            skip_quota_check: self.args.skip_quota_check,
            base_url: self.args.base_url.as_deref(),
            upstream_no_proxy: self.args.no_proxy,
            include_code_review: self.include_code_review,
            smart_context_enabled: self.args.smart_context,
            model_context_window_tokens: self.model_context_window_tokens,
            force_runtime_proxy: false,
            model_provider_override: self.model_provider_override.as_deref(),
        }
    }

    fn build_plan(
        &self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan> {
        let runtime_args = runtime_proxy_codex_passthrough_args(runtime_proxy, &self.codex_args);
        let caveman_home = prepare_caveman_launch_home(&prepared.paths, &prepared.codex_home)?;
        if let Some(mem_mode) = self.mem_mode {
            ensure_runtime_mem_prodex_observer(&prepared.paths)?;
            ensure_runtime_mem_codex_watch_for_home_with_mode(&caveman_home, mem_mode)?;
            prodex_caveman_assets::trust_claude_mem_codex_plugin_hooks(&caveman_home)?;
        }
        let mut child = codex_child_plan(caveman_home.clone(), runtime_args);
        if self.args.no_proxy && runtime_proxy.is_none() {
            remove_upstream_proxy_env(&mut child);
        }
        Ok(RuntimeLaunchPlan::new(child).with_cleanup_path(caveman_home))
    }
}

pub(super) fn handle_caveman(args: CavemanArgs) -> Result<()> {
    execute_runtime_launch(CavemanLaunchStrategy::new(args))
}

pub(super) fn prepare_caveman_launch_home(
    paths: &AppPaths,
    base_codex_home: &Path,
) -> Result<PathBuf> {
    prodex_caveman_assets::prepare_caveman_launch_home(
        &paths.managed_profiles_root,
        base_codex_home,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn super_as_caveman_args(args: &[&str]) -> CavemanArgs {
        let command =
            parse_cli_command_from(args.iter().copied()).expect("super command should parse");
        let Commands::Super(args) = command else {
            panic!("expected super command");
        };
        args.into_caveman_args()
    }

    #[test]
    fn super_default_mem_uses_super_slim_transcript_mode() {
        let strategy =
            CavemanLaunchStrategy::new(super_as_caveman_args(&["prodex", "super", "exec", "hi"]));

        assert_eq!(strategy.mem_mode, Some(RuntimeMemTranscriptMode::SuperSlim));
        assert_eq!(
            strategy.codex_args,
            vec![
                OsString::from("--dangerously-bypass-approvals-and-sandbox"),
                OsString::from("exec"),
                OsString::from("hi")
            ]
        );
    }

    #[test]
    fn super_alias_default_mem_uses_super_slim_transcript_mode() {
        let strategy =
            CavemanLaunchStrategy::new(super_as_caveman_args(&["prodex", "s", "exec", "hi"]));

        assert_eq!(strategy.mem_mode, Some(RuntimeMemTranscriptMode::SuperSlim));
    }

    #[test]
    fn super_mem_full_keeps_full_transcript_mode() {
        let strategy = CavemanLaunchStrategy::new(super_as_caveman_args(&[
            "prodex",
            "super",
            "--mem-full",
            "exec",
            "hi",
        ]));

        assert_eq!(strategy.mem_mode, Some(RuntimeMemTranscriptMode::Full));
    }

    #[test]
    fn caveman_default_mem_keeps_slim_transcript_mode() {
        let strategy = CavemanLaunchStrategy::new(CavemanArgs {
            profile: None,
            auto_rotate: false,
            no_auto_rotate: false,
            skip_quota_check: false,
            full_access: false,
            dry_run: false,
            base_url: None,
            no_proxy: false,
            smart_context: false,
            codex_args: vec![OsString::from("mem"), OsString::from("exec")],
        });

        assert_eq!(strategy.mem_mode, Some(RuntimeMemTranscriptMode::Slim));
    }
}
