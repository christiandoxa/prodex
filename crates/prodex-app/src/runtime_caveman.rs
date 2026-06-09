use super::*;
#[cfg(test)]
pub(crate) use prodex_caveman_assets::PRODEX_CAVEMAN_FULL_ASSETS_ENV;

const PRODEX_PROVIDER_CODEX_API_KEY: &str = "prodex-runtime-provider";

pub(crate) struct CavemanLaunchStrategy {
    args: CavemanArgs,
    codex_args: Vec<OsString>,
    include_code_review: bool,
    mem_mode: Option<RuntimeMemTranscriptMode>,
    rtk_enabled: bool,
    presidio_enabled: bool,
    model_provider_override: Option<String>,
    profile_v2_name: Option<String>,
    model_context_window_tokens: Option<u64>,
    gemini_thinking_budget_tokens: Option<u64>,
}

impl CavemanLaunchStrategy {
    pub(crate) fn new(args: CavemanArgs) -> Self {
        let (mem_mode, rtk_enabled, super_optimizer_prefix_enabled, codex_args) =
            runtime_caveman_extract_launch_prefixes(&args.codex_args);
        let (presidio_enabled, codex_args) = runtime_caveman_extract_presidio_prefix(codex_args);
        let mut args = args;
        args.super_optimizer_overlay |= super_optimizer_prefix_enabled;
        let mem_mode = if args.smart_context {
            runtime_mem_super_default_transcript_mode(mem_mode)
        } else {
            mem_mode
        };
        let (codex_args, include_code_review) =
            prepare_codex_launch_args(&codex_args, args.full_access);
        let model_provider_override =
            codex_cli_config_override_value(&codex_args, "model_provider");
        let profile_v2_name = codex_cli_profile_v2_name(&codex_args);
        let model_context_window_tokens =
            runtime_launch_cli_model_context_window_tokens(&codex_args);
        let gemini_thinking_budget_tokens =
            runtime_launch_cli_gemini_thinking_budget_tokens(&codex_args);
        Self {
            args,
            codex_args,
            include_code_review,
            mem_mode,
            rtk_enabled,
            presidio_enabled,
            model_provider_override,
            profile_v2_name,
            model_context_window_tokens,
            gemini_thinking_budget_tokens,
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
            presidio_redaction_enabled: self.presidio_enabled,
            model_context_window_tokens: self.model_context_window_tokens,
            gemini_thinking_budget_tokens: self.gemini_thinking_budget_tokens,
            force_runtime_proxy: false,
            model_provider_override: self.model_provider_override.as_deref(),
            profile_v2_name: self.profile_v2_name.as_deref(),
            external_provider: self
                .args
                .external_provider
                .map(SuperExternalProvider::as_str),
            external_provider_api_key: self.args.external_provider_api_key.as_deref(),
        }
    }

    fn build_plan(
        &self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan> {
        let caveman_home = prepare_caveman_launch_home(&prepared.paths, &prepared.codex_home)?;
        if self.provider_runtime_uses_local_proxy_auth() {
            write_provider_runtime_codex_auth(&caveman_home)?;
        }
        let codex_args = profile_openai_compatible_codex_args(&caveman_home, &self.codex_args);
        let codex_args = prepare_local_provider_catalog_codex_args(&caveman_home, &codex_args)?;
        let codex_args = prepare_external_provider_catalog_codex_args(&caveman_home, &codex_args)?;
        let codex_args = prepare_deepseek_provider_codex_args(&caveman_home, &codex_args)?;
        let codex_args = prepare_gemini_provider_codex_args(&caveman_home, &codex_args)?;
        let runtime_args = runtime_proxy_codex_passthrough_args(runtime_proxy, &codex_args);
        if self.rtk_enabled {
            prodex_caveman_assets::configure_rtk_codex_home(&caveman_home)?;
        }
        if self.args.super_optimizer_overlay {
            prodex_caveman_assets::configure_super_optimizer_codex_home(&caveman_home)?;
        }
        if let Some(mem_mode) = self.mem_mode {
            ensure_runtime_mem_prodex_observer(&prepared.paths)?;
            ensure_runtime_mem_codex_watch_for_home_with_mode(&caveman_home, mem_mode)?;
            prodex_caveman_assets::trust_claude_mem_codex_plugin_hooks(&caveman_home)?;
        }
        let mut child = codex_child_plan(caveman_home.clone(), runtime_args);
        if self.provider_runtime_uses_local_proxy_auth() {
            force_codex_api_key_auth_for_provider_runtime(&mut child);
        }
        prepend_child_path(&mut child, caveman_home.join("bin"));
        if self.rtk_enabled {
            clear_rtk_auto_wrap_control_env(&mut child);
        }
        if self.args.no_proxy && runtime_proxy.is_none() {
            remove_upstream_proxy_env(&mut child);
        }
        if self.presidio_enabled {
            child.extra_env.push((
                OsString::from("PRODEX_PRESIDIO_ENABLED"),
                OsString::from("1"),
            ));
        }
        Ok(RuntimeLaunchPlan::new(child).with_cleanup_path(caveman_home))
    }
}

impl CavemanLaunchStrategy {
    fn provider_runtime_uses_local_proxy_auth(&self) -> bool {
        self.args.external_provider.is_some()
            || self.model_provider_override.as_deref() == Some(SUPER_LOCAL_PROVIDER_ID)
    }
}

fn force_codex_api_key_auth_for_provider_runtime(child: &mut ChildProcessPlan) {
    let key = OsString::from("OPENAI_API_KEY");
    if let Some((_, value)) = child.extra_env.iter_mut().find(|(name, _)| name == &key) {
        *value = OsString::from(PRODEX_PROVIDER_CODEX_API_KEY);
    } else {
        child
            .extra_env
            .push((key, OsString::from(PRODEX_PROVIDER_CODEX_API_KEY)));
    }
}

fn write_provider_runtime_codex_auth(codex_home: &std::path::Path) -> Result<()> {
    let auth_path = codex_home.join("auth.json");
    let auth_json = serde_json::json!({
        "auth_mode": "apikey",
        "OPENAI_API_KEY": PRODEX_PROVIDER_CODEX_API_KEY,
        "tokens": null,
        "last_refresh": null,
        "agent_identity": null
    });
    let bytes = serde_json::to_vec_pretty(&auth_json)?;
    std::fs::write(&auth_path, bytes)
        .with_context(|| format!("failed to write {}", auth_path.display()))?;
    Ok(())
}

pub(super) fn clear_rtk_auto_wrap_control_env(child: &mut ChildProcessPlan) {
    let mut removed = BTreeSet::<OsString>::from_iter(child.removed_env.iter().cloned());
    removed.insert(OsString::from("PRODEX_RTK_AUTO_WRAP_DEPTH"));
    removed.insert(OsString::from("PRODEX_RTK_DISABLE_AUTO_WRAP"));
    child.removed_env = removed.into_iter().collect();
}

pub(super) fn prepend_child_path(child: &mut ChildProcessPlan, path: PathBuf) {
    if !path.is_dir() {
        return;
    }
    let mut paths = vec![path];
    if let Some(existing) = env::var_os("PATH") {
        paths.extend(env::split_paths(&existing));
    }
    if let Ok(joined) = env::join_paths(paths) {
        child.extra_env.push((OsString::from("PATH"), joined));
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

pub(crate) fn runtime_caveman_extract_launch_prefixes(
    args: &[OsString],
) -> (Option<RuntimeMemTranscriptMode>, bool, bool, Vec<OsString>) {
    let mut mem_mode = None;
    let mut rtk_enabled = false;
    let mut super_optimizer_overlay = false;
    let mut remaining = args.to_vec();

    loop {
        if let Some(prefix) = remaining.first().and_then(|arg| arg.to_str()) {
            if prefix == "rtk" {
                rtk_enabled = true;
                remaining.remove(0);
                continue;
            }
            if runtime_caveman_super_optimizer_prefix(prefix) {
                super_optimizer_overlay = true;
                remaining.remove(0);
                continue;
            }
        }

        let (next_mem_mode, next_remaining) = runtime_mem_extract_mode_with_detail(&remaining);
        if next_mem_mode.is_some() && next_remaining.len() != remaining.len() {
            mem_mode = next_mem_mode;
            remaining = next_remaining;
            continue;
        }

        break;
    }

    (mem_mode, rtk_enabled, super_optimizer_overlay, remaining)
}

pub(crate) fn runtime_caveman_extract_presidio_prefix(
    args: Vec<OsString>,
) -> (bool, Vec<OsString>) {
    let mut enabled = false;
    let mut remaining = Vec::with_capacity(args.len());
    for arg in args {
        if arg.to_str() == Some("presidio") {
            enabled = true;
        } else {
            remaining.push(arg);
        }
    }
    (enabled, remaining)
}

fn runtime_caveman_super_optimizer_prefix(prefix: &str) -> bool {
    matches!(
        prefix,
        "sqz" | "tokensavior" | "token-savior" | "clawcompactor" | "claw-compactor"
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
        args.into_caveman_args_with_presidio(true)
    }

    fn assert_super_optional_stack(strategy: &CavemanLaunchStrategy) {
        assert_eq!(strategy.mem_mode, Some(RuntimeMemTranscriptMode::SuperSlim));
        assert!(strategy.rtk_enabled);
        assert!(strategy.presidio_enabled);
        assert!(strategy.args.super_optimizer_overlay);
        assert!(strategy.args.smart_context);
        assert!(strategy.args.full_access);
        assert!(strategy.codex_args.contains(&OsString::from(
            "--dangerously-bypass-approvals-and-sandbox"
        )));
        for extracted_prefix in [
            "mem",
            "mem-super-slim",
            "rtk",
            "sqz",
            "tokensavior",
            "clawcompactor",
            "presidio",
        ] {
            assert!(
                !strategy
                    .codex_args
                    .contains(&OsString::from(extracted_prefix)),
                "{extracted_prefix} should be consumed before Codex launch"
            );
        }
    }

    #[test]
    fn super_default_mem_uses_super_slim_transcript_mode() {
        let strategy =
            CavemanLaunchStrategy::new(super_as_caveman_args(&["prodex", "super", "exec", "hi"]));

        assert_eq!(strategy.mem_mode, Some(RuntimeMemTranscriptMode::SuperSlim));
        assert!(strategy.rtk_enabled);
        assert!(strategy.presidio_enabled);
        assert!(strategy.args.super_optimizer_overlay);
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
        assert!(strategy.rtk_enabled);
        assert!(strategy.presidio_enabled);
        assert!(strategy.args.super_optimizer_overlay);
    }

    #[test]
    fn super_alias_keeps_optional_stack_for_default_openai_provider() {
        let strategy =
            CavemanLaunchStrategy::new(super_as_caveman_args(&["prodex", "s", "exec", "hi"]));

        assert_super_optional_stack(&strategy);
        assert!(!strategy.args.skip_quota_check);
        assert_eq!(strategy.args.external_provider, None);
        assert_eq!(strategy.model_provider_override, None);
    }

    #[test]
    fn super_alias_keeps_optional_stack_for_deepseek_provider() {
        let strategy = CavemanLaunchStrategy::new(super_as_caveman_args(&[
            "prodex",
            "s",
            "--provider",
            "deepseek",
            "--api-key",
            "deepseek-key",
            "exec",
            "hi",
        ]));

        assert_super_optional_stack(&strategy);
        assert!(strategy.args.skip_quota_check);
        assert_eq!(
            strategy.args.external_provider,
            Some(SuperExternalProvider::DeepSeek)
        );
        assert_eq!(
            strategy.args.external_provider_api_key.as_deref(),
            Some("deepseek-key")
        );
        assert_eq!(
            strategy.model_provider_override.as_deref(),
            Some("prodex-deepseek")
        );
        assert!(strategy.provider_runtime_uses_local_proxy_auth());
    }

    #[test]
    fn super_alias_keeps_optional_stack_for_gemini_provider() {
        let strategy = CavemanLaunchStrategy::new(super_as_caveman_args(&[
            "prodex",
            "s",
            "--provider",
            "gemini",
            "--api-key",
            "gemini-key",
            "exec",
            "hi",
        ]));

        assert_super_optional_stack(&strategy);
        assert!(strategy.args.skip_quota_check);
        assert_eq!(
            strategy.args.external_provider,
            Some(SuperExternalProvider::Gemini)
        );
        assert_eq!(
            strategy.args.external_provider_api_key.as_deref(),
            Some("gemini-key")
        );
        assert_eq!(
            strategy.model_provider_override.as_deref(),
            Some("prodex-gemini")
        );
        assert!(strategy.provider_runtime_uses_local_proxy_auth());
    }

    #[test]
    fn super_provider_normalizes_bare_session_id_after_provider_config() {
        let strategy = CavemanLaunchStrategy::new(super_as_caveman_args(&[
            "prodex",
            "s",
            "--provider",
            "gemini",
            "--api-key",
            "gemini-key",
            "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
        ]));

        let rendered = strategy
            .codex_args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let resume_index = rendered
            .iter()
            .position(|arg| arg == "resume")
            .expect("bare session id should be normalized to resume");
        assert_eq!(
            rendered.get(resume_index + 1).map(String::as_str),
            Some("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9")
        );
        assert!(
            rendered[..resume_index]
                .iter()
                .any(|arg| arg == "model_provider=\"prodex-gemini\"")
        );
    }

    #[test]
    fn super_alias_keeps_optional_stack_for_local_provider() {
        let strategy = CavemanLaunchStrategy::new(super_as_caveman_args(&[
            "prodex",
            "s",
            "--url",
            "http://127.0.0.1:11434",
            "exec",
            "hi",
        ]));

        assert_super_optional_stack(&strategy);
        assert!(strategy.args.skip_quota_check);
        assert_eq!(strategy.args.external_provider, None);
        assert_eq!(
            strategy.model_provider_override.as_deref(),
            Some("prodex-local")
        );
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
        assert!(strategy.rtk_enabled);
        assert!(strategy.presidio_enabled);
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
            super_optimizer_overlay: false,
            external_provider: None,
            external_provider_api_key: None,
            codex_args: vec![OsString::from("mem"), OsString::from("exec")],
        });

        assert_eq!(strategy.mem_mode, Some(RuntimeMemTranscriptMode::Slim));
        assert!(!strategy.rtk_enabled);
        assert!(!strategy.args.super_optimizer_overlay);
    }

    #[test]
    fn caveman_launch_prefixes_extract_mem_then_rtk() {
        let (mem_mode, rtk_enabled, super_optimizer_overlay, codex_args) =
            runtime_caveman_extract_launch_prefixes(&[
                OsString::from("mem"),
                OsString::from("rtk"),
                OsString::from("--full-access"),
                OsString::from("exec"),
                OsString::from("hi"),
            ]);

        assert_eq!(mem_mode, Some(RuntimeMemTranscriptMode::Slim));
        assert!(rtk_enabled);
        assert!(!super_optimizer_overlay);
        assert_eq!(
            codex_args,
            vec![
                OsString::from("--full-access"),
                OsString::from("exec"),
                OsString::from("hi")
            ]
        );
    }

    #[test]
    fn caveman_launch_prefixes_extract_rtk_then_mem() {
        let (mem_mode, rtk_enabled, super_optimizer_overlay, codex_args) =
            runtime_caveman_extract_launch_prefixes(&[
                OsString::from("rtk"),
                OsString::from("mem-full"),
                OsString::from("exec"),
            ]);

        assert_eq!(mem_mode, Some(RuntimeMemTranscriptMode::Full));
        assert!(rtk_enabled);
        assert!(!super_optimizer_overlay);
        assert_eq!(codex_args, vec![OsString::from("exec")]);
    }

    #[test]
    fn caveman_launch_prefixes_extract_all_super_optimizers() {
        let (mem_mode, rtk_enabled, super_optimizer_overlay, codex_args) =
            runtime_caveman_extract_launch_prefixes(&[
                OsString::from("mem"),
                OsString::from("rtk"),
                OsString::from("sqz"),
                OsString::from("tokensavior"),
                OsString::from("clawcompactor"),
                OsString::from("--full-access"),
                OsString::from("exec"),
                OsString::from("hi"),
            ]);

        assert_eq!(mem_mode, Some(RuntimeMemTranscriptMode::Slim));
        assert!(rtk_enabled);
        assert!(super_optimizer_overlay);
        assert_eq!(
            codex_args,
            vec![
                OsString::from("--full-access"),
                OsString::from("exec"),
                OsString::from("hi")
            ]
        );
    }

    #[test]
    fn caveman_launch_extracts_presidio_prefix_after_super_optimizers() {
        let (mem_mode, rtk_enabled, super_optimizer_overlay, codex_args) =
            runtime_caveman_extract_launch_prefixes(&[
                OsString::from("mem"),
                OsString::from("rtk"),
                OsString::from("sqz"),
                OsString::from("presidio"),
                OsString::from("exec"),
                OsString::from("hi"),
            ]);
        let (presidio_enabled, codex_args) = runtime_caveman_extract_presidio_prefix(codex_args);

        assert_eq!(mem_mode, Some(RuntimeMemTranscriptMode::Slim));
        assert!(rtk_enabled);
        assert!(super_optimizer_overlay);
        assert!(presidio_enabled);
        assert_eq!(
            codex_args,
            vec![OsString::from("exec"), OsString::from("hi")]
        );
    }

    #[test]
    fn rtk_launch_clears_auto_wrap_control_env() {
        let mut child = ChildProcessPlan {
            binary: OsString::from("codex"),
            args: Vec::new(),
            codex_home: PathBuf::from("/tmp/prodex-caveman-test"),
            extra_env: Vec::new(),
            removed_env: vec![OsString::from("CODEX_SANDBOX")],
        };

        clear_rtk_auto_wrap_control_env(&mut child);

        assert!(
            child
                .removed_env
                .contains(&OsString::from("PRODEX_RTK_AUTO_WRAP_DEPTH"))
        );
        assert!(
            child
                .removed_env
                .contains(&OsString::from("PRODEX_RTK_DISABLE_AUTO_WRAP"))
        );
        assert!(child.removed_env.contains(&OsString::from("CODEX_SANDBOX")));
    }

    #[test]
    fn provider_runtime_auth_sets_codex_api_key_placeholder() {
        let mut child = ChildProcessPlan {
            binary: OsString::from("codex"),
            args: Vec::new(),
            codex_home: PathBuf::from("/tmp/prodex-caveman-test"),
            extra_env: vec![(OsString::from("OPENAI_API_KEY"), OsString::from("user-key"))],
            removed_env: Vec::new(),
        };

        force_codex_api_key_auth_for_provider_runtime(&mut child);

        let values = child
            .extra_env
            .iter()
            .filter(|(key, _)| key == "OPENAI_API_KEY")
            .map(|(_, value)| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec![PRODEX_PROVIDER_CODEX_API_KEY.to_string()]);
    }

    #[test]
    fn provider_runtime_auth_writes_api_key_auth_file() {
        let root = std::env::temp_dir().join(format!(
            "prodex-provider-auth-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("temp home should be created");

        write_provider_runtime_codex_auth(&root).expect("auth file should be written");

        let auth = std::fs::read_to_string(root.join("auth.json")).expect("auth should be read");
        let value: serde_json::Value = serde_json::from_str(&auth).expect("auth should be json");
        assert_eq!(value["auth_mode"], "apikey");
        assert_eq!(value["OPENAI_API_KEY"], PRODEX_PROVIDER_CODEX_API_KEY);
        assert!(value["tokens"].is_null());
        std::fs::remove_dir_all(root).expect("temp home should be removed");
    }
}
