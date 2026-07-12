use super::*;
#[cfg(test)]
pub(crate) use prodex_caveman_assets::PRODEX_CAVEMAN_FULL_ASSETS_ENV;

const PRODEX_PROVIDER_CODEX_API_KEY: &str = "prodex-runtime-provider";

pub(crate) struct CavemanLaunchStrategy {
    args: CavemanArgs,
    codex_args: Vec<OsString>,
    include_code_review: bool,
    rtk_enabled: bool,
    presidio_enabled: bool,
    model_provider_override: Option<String>,
    profile_v2_name: Option<String>,
    model_context_window_tokens: Option<u64>,
    gemini_thinking_budget_tokens: Option<u64>,
}

impl CavemanLaunchStrategy {
    pub(crate) fn new(args: CavemanArgs) -> Self {
        let codex_feature_args = args.codex_args_with_feature_overrides();
        let (rtk_enabled, super_optimizer_prefix_enabled, codex_args) =
            runtime_caveman_extract_launch_prefixes(&codex_feature_args);
        let (presidio_enabled, codex_args) = runtime_caveman_extract_presidio_prefix(codex_args);
        let mut args = args;
        args.super_optimizer_overlay |= super_optimizer_prefix_enabled;
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
        }
    }
}

impl RuntimeLaunchStrategy for CavemanLaunchStrategy {
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
        if self.presidio_enabled {
            ensure_presidio_services_for_super_launch(&prepared.paths)?;
        }
        let overlay_home = prepare_prodex_overlay_home(&prepared.paths, &prepared.codex_home)?;
        if self.provider_runtime_uses_local_proxy_auth() {
            write_provider_runtime_codex_auth(&overlay_home)?;
        }
        let codex_args = if self.args.super_optimizer_overlay {
            trusted_workspace_codex_args(&env::current_dir()?, &self.codex_args)
        } else {
            self.codex_args.clone()
        };
        let codex_args = runtime_launch_openai_spark_context_codex_args(&overlay_home, &codex_args);
        let codex_args = profile_openai_compatible_codex_args(&overlay_home, &codex_args)?;
        let codex_args = prepare_local_provider_catalog_codex_args(&overlay_home, &codex_args)?;
        let codex_args = prepare_external_provider_catalog_codex_args(&overlay_home, &codex_args)?;
        let codex_args = prepare_deepseek_provider_codex_args(&overlay_home, &codex_args)?;
        let codex_args = prepare_gemini_provider_codex_args(&overlay_home, &codex_args)?;
        let runtime_args = runtime_proxy_codex_passthrough_args(runtime_proxy, &codex_args);
        if self.rtk_enabled {
            prodex_caveman_assets::configure_rtk_codex_home(&overlay_home)?;
        }
        if self.args.super_optimizer_overlay {
            prodex_caveman_assets::configure_super_optimizer_codex_home_with_presidio(
                &overlay_home,
                self.presidio_enabled,
            )?;
        }
        let mut child = codex_child_plan(overlay_home.clone(), runtime_args);
        if self.provider_runtime_uses_local_proxy_auth() {
            force_codex_api_key_auth_for_provider_runtime(&mut child);
            remove_provider_secret_env(&mut child);
        }
        prepend_child_path(&mut child, overlay_home.join("bin"));
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
        Ok(RuntimeLaunchPlan::new(child).with_cleanup_path(overlay_home))
    }
}

fn trusted_workspace_codex_args(workspace: &Path, codex_args: &[OsString]) -> Vec<OsString> {
    let workspace = serde_json::to_string(&workspace.to_string_lossy())
        .expect("workspace path should serialize as a TOML-compatible string");
    let mut args = Vec::with_capacity(codex_args.len() + 2);
    args.push(OsString::from("-c"));
    args.push(OsString::from(format!(
        "projects={{{workspace}={{trust_level=\"trusted\"}}}}"
    )));
    args.extend(codex_args.iter().cloned());
    args
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

pub(super) fn handle_caveman(args: CavemanArgs) -> Result<()> {
    if let Some(base_url) = args.base_url.as_deref() {
        validate_credential_free_http_url(base_url, "runtime upstream base URL")?;
    }
    execute_runtime_launch(CavemanLaunchStrategy::new(args))
}

pub(super) fn prepare_prodex_overlay_home(
    paths: &AppPaths,
    base_codex_home: &Path,
) -> Result<PathBuf> {
    let sessions_are_managed = prodex_core::same_path(
        &base_codex_home.join("sessions"),
        &paths.shared_codex_root.join("sessions"),
    );
    if sessions_are_managed {
        // Recheck fingerprints immediately before linking history so concurrent session updates
        // retain the same attachment-persistence behavior without rescanning every JSONL payload.
        prodex_shared_codex_fs::maintain_managed_codex_sessions(paths)?;
        return prodex_caveman_assets::prepare_prodex_overlay_home_from_prepared_base(
            &paths.managed_profiles_root,
            base_codex_home,
        );
    }
    prodex_caveman_assets::prepare_prodex_overlay_home(
        &paths.managed_profiles_root,
        base_codex_home,
    )
}

pub(crate) fn runtime_caveman_extract_launch_prefixes(
    args: &[OsString],
) -> (bool, bool, Vec<OsString>) {
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
            if prefix == "caveman" {
                remaining.remove(0);
                continue;
            }
            if runtime_caveman_super_optimizer_prefix(prefix) {
                super_optimizer_overlay = true;
                remaining.remove(0);
                continue;
            }
        }

        break;
    }

    (rtk_enabled, super_optimizer_overlay, remaining)
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
    prefix == "ponytail"
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
        assert!(strategy.rtk_enabled);
        assert!(strategy.presidio_enabled);
        assert!(strategy.args.super_optimizer_overlay);
        assert!(strategy.args.smart_context);
        assert!(strategy.runtime_request().smart_context_enabled);
        assert!(strategy.args.full_access);
        assert!(strategy.codex_args.contains(&OsString::from(
            "--dangerously-bypass-approvals-and-sandbox"
        )));
        assert!(
            strategy
                .codex_args
                .contains(&OsString::from("--dangerously-bypass-hook-trust"))
        );
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
    fn super_default_enables_optimizer_stack() {
        let strategy =
            CavemanLaunchStrategy::new(super_as_caveman_args(&["prodex", "super", "exec", "hi"]));

        assert!(strategy.rtk_enabled);
        assert!(strategy.presidio_enabled);
        assert!(strategy.args.super_optimizer_overlay);
        assert_eq!(
            strategy.codex_args,
            vec![
                OsString::from("--dangerously-bypass-approvals-and-sandbox"),
                OsString::from("--dangerously-bypass-hook-trust"),
                OsString::from("exec"),
                OsString::from("hi")
            ]
        );
    }

    #[test]
    fn super_trusts_workspace_without_persisting_config() {
        let args = trusted_workspace_codex_args(
            Path::new("/tmp/project"),
            &[OsString::from("--dangerously-bypass-approvals-and-sandbox")],
        );
        assert_eq!(
            args,
            vec![
                OsString::from("-c"),
                OsString::from("projects={\"/tmp/project\"={trust_level=\"trusted\"}}"),
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
            CavemanLaunchStrategy::new(super_as_caveman_args(&["prodex", "s", "exec", "hi"]));

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
    fn super_alias_keeps_optional_stack_for_copilot_provider() {
        let strategy = CavemanLaunchStrategy::new(super_as_caveman_args(&[
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
    fn caveman_launch_prefixes_extract_minimal_super_stack() {
        let (rtk_enabled, super_optimizer_overlay, codex_args) =
            runtime_caveman_extract_launch_prefixes(&[
                OsString::from("rtk"),
                OsString::from("ponytail"),
                OsString::from("--full-access"),
                OsString::from("exec"),
                OsString::from("hi"),
            ]);

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
    fn caveman_launch_prefixes_allow_rtk_caveman_ponytail_combo() {
        let (rtk_enabled, super_optimizer_overlay, codex_args) =
            runtime_caveman_extract_launch_prefixes(&[
                OsString::from("rtk"),
                OsString::from("caveman"),
                OsString::from("ponytail"),
                OsString::from("exec"),
                OsString::from("hi"),
            ]);

        assert!(rtk_enabled);
        assert!(super_optimizer_overlay);
        assert_eq!(
            codex_args,
            vec![OsString::from("exec"), OsString::from("hi")]
        );
    }

    #[test]
    fn caveman_launch_extracts_presidio_prefix_after_super_optimizers() {
        let (rtk_enabled, super_optimizer_overlay, codex_args) =
            runtime_caveman_extract_launch_prefixes(&[
                OsString::from("rtk"),
                OsString::from("ponytail"),
                OsString::from("presidio"),
                OsString::from("exec"),
                OsString::from("hi"),
            ]);
        let (presidio_enabled, codex_args) = runtime_caveman_extract_presidio_prefix(codex_args);

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
