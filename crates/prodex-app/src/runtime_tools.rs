use super::*;
use crate::runtime_desktop::{
    DesktopGuiCommand, configure_desktop_codex_home, desktop_gui_command,
    prepare_desktop_overlay_home, prepare_runtime_overlay_home, repair_desktop_thread_index,
};
#[path = "runtime_tools/overlay.rs"]
mod overlay;
#[path = "runtime_tools/sub_agents.rs"]
mod sub_agents;
#[path = "runtime_tools/super_trust.rs"]
mod super_trust;
#[cfg(test)]
pub(super) use overlay::prepare_prodex_overlay_home;
pub(crate) use overlay::resolve_runtime_optional_tool_plan;
pub(crate) use sub_agents::*;
pub(crate) use super_trust::trusted_workspace_codex_args;
const PRODEX_PROVIDER_CODEX_API_KEY: &str = "prodex-runtime-provider";

pub(crate) struct RuntimeToolLaunchStrategy {
    args: RuntimeToolArgs,
    codex_args: Vec<OsString>,
    include_code_review: bool,
    rtk_enabled: bool,
    presidio_enabled: bool,
    model_provider_override: Option<String>,
    profile_v2_name: Option<String>,
    model_context_window_tokens: Option<u64>,
    gemini_thinking_budget_tokens: Option<u64>,
    desktop_command: Option<DesktopGuiCommand>,
    configure_prodex_overlay: bool,
    sub_agent: Option<ResolvedSuperSubAgent>,
}

impl RuntimeToolLaunchStrategy {
    pub(crate) fn new(args: RuntimeToolArgs) -> Self {
        Self::new_with_sub_agent(args, None)
    }

    pub(crate) fn new_with_sub_agent(
        args: RuntimeToolArgs,
        sub_agent: Option<ResolvedSuperSubAgent>,
    ) -> Self {
        let codex_feature_args = args.codex_args_with_feature_overrides();
        let selected_tools = args.selected_tool_set();
        let rtk_enabled = selected_tools.contains(prodex_optional_tools::OptionalToolId::Rtk);
        let presidio_enabled = args.presidio
            || selected_tools.contains(prodex_optional_tools::OptionalToolId::Presidio);
        let codex_args = codex_feature_args;
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
            rtk_enabled,
            presidio_enabled,
            model_provider_override,
            profile_v2_name,
            model_context_window_tokens,
            gemini_thinking_budget_tokens,
            desktop_command: None,
            configure_prodex_overlay: true,
            sub_agent,
        }
    }

    pub(crate) fn new_desktop(
        args: RuntimeToolArgs,
        configure_prodex_overlay: bool,
    ) -> Result<Self> {
        let mut strategy = Self::new(args);
        strategy.desktop_command = Some(desktop_gui_command()?);
        strategy.configure_prodex_overlay = configure_prodex_overlay;
        Ok(strategy)
    }
}

impl RuntimeLaunchStrategy for RuntimeToolLaunchStrategy {
    fn harness_mode(&self) -> Option<prodex_provider_core::HarnessMode> {
        self.args.harness
    }

    fn runtime_request(&self) -> RuntimeLaunchRequest<'_> {
        RuntimeLaunchRequest {
            profile: self.args.profile.as_deref(),
            allow_auto_rotate: !self.args.no_auto_rotate,
            auto_redeem: self.args.auto_redeem,
            skip_quota_check: self.args.skip_quota_check,
            base_url: self.args.base_url.as_deref(),
            upstream_no_proxy: self.args.no_proxy,
            include_code_review: self.include_code_review,
            smart_context_enabled: self.args.smart_context,
            presidio_redaction_enabled: self.presidio_enabled,
            model_context_window_tokens: self.model_context_window_tokens,
            gemini_thinking_budget_tokens: self.gemini_thinking_budget_tokens,
            force_runtime_proxy: self.desktop_command.is_some(),
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
        overlay::build_plan(self, prepared, runtime_proxy)
    }
}

impl RuntimeToolLaunchStrategy {
    fn prepare_runtime_codex_args(
        &self,
        overlay_home: &std::path::Path,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<Vec<OsString>> {
        let codex_args = if self.args.super_mode {
            trusted_workspace_codex_args(&env::current_dir()?, &self.codex_args)
        } else {
            self.codex_args.clone()
        };
        let codex_args = runtime_launch_openai_spark_context_codex_args(overlay_home, &codex_args)?;
        let codex_args = profile_openai_compatible_codex_args(overlay_home, &codex_args)?;
        let codex_args = prepare_local_provider_catalog_codex_args(overlay_home, &codex_args)?;
        let codex_args = prepare_external_provider_catalog_codex_args(overlay_home, &codex_args)?;
        let codex_args = prepare_deepseek_provider_codex_args(overlay_home, &codex_args)?;
        let codex_args = prepare_gemini_provider_codex_args(overlay_home, &codex_args)?;
        Ok(runtime_proxy_codex_passthrough_args(
            runtime_proxy,
            &codex_args,
        ))
    }

    fn build_child_plan(
        &self,
        prepared: &PreparedRuntimeLaunch,
        overlay_home: &std::path::Path,
        runtime_args: &[OsString],
    ) -> Result<ChildProcessPlan> {
        if let Some(desktop) = self.desktop_command.as_ref() {
            let sqlite_home = prepared
                .managed
                .then_some(prepared.paths.shared_codex_root.as_path());
            configure_desktop_codex_home(
                overlay_home,
                runtime_args,
                self.args.full_access,
                sqlite_home,
            )?;
            if !self.args.dry_run
                && let Some(sqlite_home) = sqlite_home
            {
                repair_desktop_thread_index(&crate::codex_bin(), overlay_home, sqlite_home)?;
            }
            let mut child = codex_child_plan(overlay_home.to_path_buf(), Vec::new());
            child.binary = desktop.binary.clone();
            child.args = desktop.args.clone();
            Ok(child)
        } else {
            Ok(codex_tui_child_plan(
                overlay_home.to_path_buf(),
                runtime_args.to_vec(),
            ))
        }
    }

    fn finalize_child_plan(
        &self,
        child: &mut ChildProcessPlan,
        overlay_home: &std::path::Path,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) {
        if self.provider_runtime_uses_local_proxy_auth() {
            force_codex_api_key_auth_for_provider_runtime(child);
            remove_provider_secret_env(child);
        }
        prepend_child_path(child, overlay_home.join("bin"));
        if self.rtk_enabled {
            clear_rtk_auto_wrap_control_env(child);
        }
        if self.args.no_proxy && runtime_proxy.is_none() {
            remove_upstream_proxy_env(child);
        }
        if self.presidio_enabled {
            child.extra_env.push((
                OsString::from("PRODEX_PRESIDIO_ENABLED"),
                OsString::from("1"),
            ));
        }
        apply_sub_agent_recursion_marker(child, self.sub_agent.as_ref());
    }
}

impl RuntimeToolLaunchStrategy {
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
    prodex_shared_codex_fs::create_codex_home_if_missing(codex_home)?;
    let auth_path = codex_home.join("auth.json");
    let auth_json = serde_json::json!({
        "auth_mode": "apikey",
        "OPENAI_API_KEY": PRODEX_PROVIDER_CODEX_API_KEY,
        "tokens": null,
        "last_refresh": null,
        "agent_identity": null
    });
    let text = serde_json::to_string_pretty(&auth_json)?;
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(&auth_path), text)
        .map_err(anyhow::Error::new)
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

pub(super) fn handle_runtime_tools(args: RuntimeToolArgs) -> Result<()> {
    if let Some(base_url) = args.base_url.as_deref() {
        validate_credential_free_http_url(base_url, "runtime upstream base URL")?;
    }
    execute_runtime_launch(RuntimeToolLaunchStrategy::new(args))
}

pub(crate) fn handle_super_runtime_tools(
    args: RuntimeToolArgs,
    sub_agent: Option<ResolvedSuperSubAgent>,
) -> Result<()> {
    if let Some(base_url) = args.base_url.as_deref() {
        validate_credential_free_http_url(base_url, "runtime upstream base URL")?;
    }
    execute_runtime_launch(RuntimeToolLaunchStrategy::new_with_sub_agent(
        args, sub_agent,
    ))
}

pub(crate) fn handle_super_runtime_tools_dry_run(
    args: SuperArgs,
    presidio: bool,
    sub_agent: Option<&ResolvedSuperSubAgent>,
) -> Result<()> {
    let args = args.into_runtime_tool_args_with_presidio(presidio);
    if let Some(base_url) = args.base_url.as_deref() {
        validate_credential_free_http_url(base_url, "runtime upstream base URL")?;
    }
    let selected_tools = args.selected_tool_set();
    let required_tools = args.required_tool_set();
    let presidio_enabled =
        args.presidio || selected_tools.contains(prodex_optional_tools::OptionalToolId::Presidio);
    let tool_plan = resolve_runtime_optional_tool_plan(&selected_tools, &required_tools)?;
    let codex_args = args.codex_args_with_feature_overrides();
    let (_, codex_args) = extract_prodex_dry_run_flag(&codex_args);
    let (codex_args, include_code_review) =
        prepare_codex_launch_args(&codex_args, args.full_access);
    let codex_args = if args.super_mode {
        trusted_workspace_codex_args(&std::env::current_dir()?, &codex_args)
    } else {
        codex_args
    };
    let codex_args = redact_super_session_args(&codex_args);
    let model_provider_override = codex_cli_config_override_value(&codex_args, "model_provider");
    let profile_v2_name = codex_cli_profile_v2_name(&codex_args);
    let model_context_window_tokens = runtime_launch_cli_model_context_window_tokens(&codex_args);
    let gemini_thinking_budget_tokens =
        runtime_launch_cli_gemini_thinking_budget_tokens(&codex_args);
    let resolved_harness = prodex_provider_core::resolve_harness_mode(args.harness, None);
    let request = RuntimeLaunchRequest {
        profile: args.profile.as_deref(),
        allow_auto_rotate: !args.no_auto_rotate,
        auto_redeem: args.auto_redeem,
        skip_quota_check: args.skip_quota_check,
        base_url: args.base_url.as_deref(),
        upstream_no_proxy: args.no_proxy,
        include_code_review,
        smart_context_enabled: args.smart_context,
        presidio_redaction_enabled: presidio_enabled,
        model_context_window_tokens,
        gemini_thinking_budget_tokens,
        force_runtime_proxy: false,
        model_provider_override: model_provider_override.as_deref(),
        profile_v2_name: profile_v2_name.as_deref(),
        external_provider: args
            .external_provider
            .map(crate::SuperExternalProvider::as_str),
        external_provider_api_key: args.external_provider_api_key.as_deref(),
    };
    let mut extra_report = String::from("Optional tools:");
    for activation in &tool_plan.activations {
        extra_report.push_str(&format!(
            "\n  {}: resolved (activation deferred until launch)",
            activation.tool.descriptor.id
        ));
    }
    for unavailable in &tool_plan.unavailable {
        extra_report.push_str(&format!(
            "\n  {}: skipped ({})",
            unavailable.id,
            redaction::redaction_redact_secret_like_text(&unavailable.detail)
        ));
    }
    if selected_tools.contains(prodex_optional_tools::OptionalToolId::Presidio) {
        extra_report.push_str(&format!(
            "\n  presidio: {}",
            if presidio_enabled {
                "requested"
            } else {
                "disabled"
            }
        ));
    }
    extra_report.push_str("\nDry run: optional overlays and services are not started.\n");
    if let Some(sub_agent) = sub_agent {
        extra_report.push_str(&render_sub_agent_dry_run_report(sub_agent));
    } else {
        extra_report.push_str(&render_sub_agent_disabled_dry_run_report(presidio_enabled));
    }
    print_runtime_launch_dry_run(
        "optional-tools",
        request,
        RuntimeLaunchDryRunChild::Caveman { codex_args },
        Some(resolved_harness),
        Some(&extra_report),
    )
}

pub(super) fn handle_desktop_gui(
    args: RuntimeToolArgs,
    configure_prodex_overlay: bool,
) -> Result<()> {
    if let Some(base_url) = args.base_url.as_deref() {
        validate_credential_free_http_url(base_url, "runtime upstream base URL")?;
    }
    execute_runtime_launch(RuntimeToolLaunchStrategy::new_desktop(
        args,
        configure_prodex_overlay,
    )?)
}

#[cfg(test)]
#[path = "../tests/src/runtime_tools_desktop.rs"]
mod desktop_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn super_as_caveman_args(args: &[&str]) -> RuntimeToolArgs {
        let command =
            parse_cli_command_from(args.iter().copied()).expect("super command should parse");
        let Commands::Super(args) = command else {
            panic!("expected super command");
        };
        args.into_runtime_tool_args_with_presidio(true)
    }

    fn assert_super_optional_stack(strategy: &RuntimeToolLaunchStrategy) {
        assert!(strategy.rtk_enabled);
        assert!(strategy.presidio_enabled);
        assert!(
            strategy
                .args
                .selected_tool_set()
                .contains(prodex_optional_tools::OptionalToolId::Ponytail)
        );
        assert!(strategy.args.smart_context);
        assert!(strategy.args.super_mode);
        assert!(strategy.runtime_request().smart_context_enabled);
        assert!(strategy.args.full_access);
        assert!(strategy.codex_args.contains(&OsString::from(
            "--dangerously-bypass-approvals-and-sandbox"
        )));
        for extracted_prefix in ["rtk", "ponytail", "presidio"] {
            assert!(
                !strategy
                    .codex_args
                    .contains(&OsString::from(extracted_prefix)),
                "{extracted_prefix} should be consumed before Codex launch"
            );
        }
    }

    #[test]
    fn super_default_enables_optimizer_stack_with_yolo_access() {
        let strategy = RuntimeToolLaunchStrategy::new(super_as_caveman_args(&[
            "prodex", "super", "exec", "hi",
        ]));

        assert!(strategy.rtk_enabled);
        assert!(strategy.presidio_enabled);
        assert!(
            strategy
                .args
                .selected_tool_set()
                .contains(prodex_optional_tools::OptionalToolId::CodebaseMemoryMcp)
        );
        assert_eq!(
            strategy.codex_args,
            vec![
                OsString::from("--dangerously-bypass-approvals-and-sandbox"),
                OsString::from("-c"),
                OsString::from("features.apps=false"),
                OsString::from("exec"),
                OsString::from("hi")
            ]
        );
    }

    #[test]
    fn desktop_strategy_forces_runtime_proxy() {
        let command =
            parse_cli_command_from(["prodex", "caveman"]).expect("caveman command should parse");
        let Commands::Caveman(args) = command else {
            panic!("expected caveman command");
        };
        let mut strategy = RuntimeToolLaunchStrategy::new(args);
        assert!(!strategy.runtime_request().force_runtime_proxy);

        strategy.desktop_command = Some(DesktopGuiCommand {
            binary: OsString::from("desktop"),
            args: Vec::new(),
        });

        assert!(strategy.runtime_request().force_runtime_proxy);
    }

    #[test]
    fn super_trusts_workspace_and_hooks_without_persisting_config() {
        let args = trusted_workspace_codex_args(
            Path::new("/tmp/project"),
            &[OsString::from("--dangerously-bypass-approvals-and-sandbox")],
        );
        assert_eq!(
            args,
            vec![
                OsString::from("-c"),
                OsString::from("projects={\"/tmp/project\"={trust_level=\"trusted\"}}"),
                OsString::from("--dangerously-bypass-hook-trust"),
                OsString::from("--dangerously-bypass-approvals-and-sandbox"),
            ]
        );
        let config: toml::Value =
            toml::from_str(args[1].to_str().expect("config override should be UTF-8"))
                .expect("config override should be valid TOML");
        assert_eq!(
            config["projects"]["/tmp/project"]["trust_level"].as_str(),
            Some("trusted")
        );
    }

    #[test]
    fn super_alias_enables_optimizer_stack() {
        let strategy =
            RuntimeToolLaunchStrategy::new(super_as_caveman_args(&["prodex", "s", "exec", "hi"]));

        assert!(strategy.rtk_enabled);
        assert!(strategy.presidio_enabled);
        assert!(
            strategy
                .args
                .selected_tool_set()
                .contains(prodex_optional_tools::OptionalToolId::Ponytail)
        );
    }

    #[test]
    fn super_alias_keeps_optional_stack_for_default_openai_provider() {
        let strategy =
            RuntimeToolLaunchStrategy::new(super_as_caveman_args(&["prodex", "s", "exec", "hi"]));

        assert_super_optional_stack(&strategy);
        assert!(!strategy.args.skip_quota_check);
        assert_eq!(strategy.args.external_provider, None);
        assert_eq!(strategy.model_provider_override, None);
    }

    #[test]
    fn super_alias_keeps_optional_stack_for_deepseek_provider() {
        let strategy = RuntimeToolLaunchStrategy::new(super_as_caveman_args(&[
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
        let strategy = RuntimeToolLaunchStrategy::new(super_as_caveman_args(&[
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
    fn super_alias_keeps_optional_stack_for_copilot_provider() {
        let strategy = RuntimeToolLaunchStrategy::new(super_as_caveman_args(&[
            "prodex",
            "s",
            "--provider",
            "copilot",
            "--api-key",
            "copilot-key",
            "exec",
            "hi",
        ]));

        assert_super_optional_stack(&strategy);
        assert!(strategy.args.skip_quota_check);
        assert_eq!(
            strategy.args.external_provider,
            Some(SuperExternalProvider::Copilot)
        );
        assert_eq!(
            strategy.args.external_provider_api_key.as_deref(),
            Some("copilot-key")
        );
        assert_eq!(
            strategy.model_provider_override.as_deref(),
            Some("prodex-copilot")
        );
        assert!(strategy.provider_runtime_uses_local_proxy_auth());
        assert!(
            strategy
                .codex_args
                .iter()
                .any(|arg| arg.to_string_lossy() == "web_search=\"live\"")
        );
    }

    #[test]
    fn super_provider_normalizes_bare_session_id_after_provider_config() {
        let strategy = RuntimeToolLaunchStrategy::new(super_as_caveman_args(&[
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
        assert_eq!(
            prodex_runtime_launch::codex_resume_session_id(&strategy.codex_args),
            Some("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9")
        );
        assert!(
            rendered[..resume_index]
                .iter()
                .any(|arg| arg == "model_provider=\"prodex-gemini\"")
        );
    }

    #[cfg(unix)]
    #[test]
    fn provider_runtime_codex_auth_is_written_private() {
        use std::os::unix::fs::PermissionsExt;

        let codex_home = env::temp_dir().join(format!(
            "prodex-caveman-auth-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        create_codex_home_if_missing(&codex_home).expect("codex home should exist");

        write_provider_runtime_codex_auth(&codex_home).expect("provider auth should write");

        let auth_path = codex_home.join("auth.json");
        let mode = std::fs::metadata(&auth_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let _ = std::fs::remove_dir_all(codex_home);
    }

    #[test]
    fn super_alias_keeps_optional_stack_for_local_provider() {
        let strategy = RuntimeToolLaunchStrategy::new(super_as_caveman_args(&[
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
    fn legacy_leading_tool_prefixes_translate_to_typed_selection() {
        let command = parse_cli_command_from([
            "prodex",
            "caveman",
            "rtk",
            "playwright",
            "ponytail",
            "exec",
            "hi",
        ])
        .unwrap();
        let Commands::Caveman(args) = command else {
            panic!("expected caveman command");
        };
        let tools = args.selected_tool_set();
        assert!(tools.contains(prodex_optional_tools::OptionalToolId::Rtk));
        assert!(tools.contains(prodex_optional_tools::OptionalToolId::PlaywrightMcp));
        assert!(tools.contains(prodex_optional_tools::OptionalToolId::Ponytail));
        assert_eq!(
            args.codex_args,
            [OsString::from("exec"), OsString::from("hi")]
        );
    }

    #[test]
    fn non_prefix_presidio_is_preserved_for_codex() {
        let command =
            parse_cli_command_from(["prodex", "caveman", "exec", "presidio", "hi"]).unwrap();
        let Commands::Caveman(args) = command else {
            panic!("expected caveman command");
        };
        assert!(!args.presidio);
        assert_eq!(
            args.codex_args,
            [
                OsString::from("exec"),
                OsString::from("presidio"),
                OsString::from("hi")
            ]
        );
    }

    #[test]
    fn rtk_alias_launch_keeps_approval_bypass_explicit() {
        let command = parse_cli_command_from(["prodex", "rtk", "exec", "review"])
            .expect("rtk shortcut should parse");
        let Commands::Rtk(args) = command else {
            panic!("expected rtk shortcut");
        };
        let strategy = RuntimeToolLaunchStrategy::new(runtime_tool_args_with_tool(
            args,
            prodex_optional_tools::OptionalToolId::Rtk,
        ));

        assert!(strategy.rtk_enabled);
        assert!(!strategy.args.full_access);
        assert!(!strategy.codex_args.contains(&OsString::from(
            "--dangerously-bypass-approvals-and-sandbox"
        )));
        assert!(
            !strategy
                .codex_args
                .contains(&OsString::from("--dangerously-bypass-hook-trust"))
        );
    }

    #[test]
    fn legacy_leading_presidio_translates_without_string_surgery() {
        let command = parse_cli_command_from([
            "prodex", "caveman", "rtk", "ponytail", "presidio", "exec", "hi",
        ])
        .unwrap();
        let Commands::Caveman(args) = command else {
            panic!("expected caveman command");
        };
        assert!(args.presidio);
        assert!(
            args.selected_tool_set()
                .contains(prodex_optional_tools::OptionalToolId::Presidio)
        );
        assert_eq!(
            args.codex_args,
            [OsString::from("exec"), OsString::from("hi")]
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
            reset_terminal_keyboard_enhancement: false,
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
    fn provider_runtime_auth_sets_local_placeholder_and_removes_upstream_secrets() {
        let mut child = ChildProcessPlan {
            binary: OsString::from("codex"),
            args: Vec::new(),
            codex_home: PathBuf::from("/tmp/prodex-caveman-test"),
            extra_env: vec![
                (OsString::from("OPENAI_API_KEY"), OsString::from("user-key")),
                (
                    OsString::from("UNRELATED_CHILD_ENV"),
                    OsString::from("keep-me"),
                ),
            ],
            removed_env: vec![OsString::from("EXISTING_REMOVED_ENV")],
            reset_terminal_keyboard_enhancement: false,
        };

        force_codex_api_key_auth_for_provider_runtime(&mut child);
        remove_provider_secret_env(&mut child);

        let values = child
            .extra_env
            .iter()
            .filter(|(key, _)| key == "OPENAI_API_KEY")
            .map(|(_, value)| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec![PRODEX_PROVIDER_CODEX_API_KEY.to_string()]);
        for key in PROVIDER_SECRET_ENV_KEYS {
            assert!(
                child.removed_env.contains(&OsString::from(key)),
                "provider secret env {key} should be removed"
            );
        }
        assert!(
            child
                .removed_env
                .contains(&OsString::from("EXISTING_REMOVED_ENV"))
        );
        assert!(
            !child
                .removed_env
                .contains(&OsString::from("UNRELATED_CHILD_ENV"))
        );
        assert!(
            child
                .extra_env
                .iter()
                .any(|(key, value)| { key == "UNRELATED_CHILD_ENV" && value == "keep-me" })
        );
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
