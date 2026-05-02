use super::*;
#[cfg(test)]
pub(crate) use prodex_runtime_launch::local_proxy_bypass_env;
pub(crate) use prodex_runtime_launch::{
    RuntimeLaunchDryRunChild, codex_sandbox_removed_env, extract_prodex_dry_run_flag,
    prepare_codex_launch_args, prodex_dry_run_requested, remove_upstream_proxy_env,
};

pub(crate) fn codex_child_plan(codex_home: PathBuf, args: Vec<OsString>) -> ChildProcessPlan {
    prodex_runtime_launch::codex_child_plan(codex_bin(), codex_home, args, SUPER_LOCAL_PROVIDER_ID)
}

pub(crate) fn run_child_plan(
    plan: &ChildProcessPlan,
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
) -> Result<ExitStatus> {
    let mut command = Command::new(&plan.binary);
    command.args(&plan.args).env("CODEX_HOME", &plan.codex_home);
    for key in &plan.removed_env {
        command.env_remove(key);
    }
    for (key, value) in &plan.extra_env {
        command.env(key, value);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to execute {}", plan.binary.to_string_lossy()))?;
    let _child_runtime_broker_lease = match runtime_proxy {
        Some(proxy) => match proxy.create_child_lease(child.id()) {
            Ok(lease) => Some(lease),
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(err);
            }
        },
        None => None,
    };
    let status = child
        .wait()
        .with_context(|| format!("failed to wait for {}", plan.binary.to_string_lossy()))?;
    Ok(status)
}

pub(crate) fn run_codex_direct_passthrough(args: Vec<OsString>) -> Result<ExitStatus> {
    let binary = codex_bin();
    let mut command = Command::new(&binary);
    command.args(args);
    for key in codex_sandbox_removed_env() {
        command.env_remove(key);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to execute {}", binary.to_string_lossy()))?;
    let status = child
        .wait()
        .with_context(|| format!("failed to wait for {}", binary.to_string_lossy()))?;
    Ok(status)
}

pub(crate) fn handle_codex_update(args: CodexUpdateArgs) -> Result<()> {
    let mut command_args = Vec::with_capacity(args.codex_args.len() + 1);
    command_args.push(OsString::from("update"));
    command_args.extend(args.codex_args);
    exit_with_status(run_codex_direct_passthrough(command_args)?)
}

pub(crate) fn exit_with_status(status: ExitStatus) -> Result<()> {
    std::process::exit(status.code().unwrap_or(1));
}

pub(crate) fn handle_caveman_dry_run(args: CavemanArgs) -> Result<()> {
    let (_mem_mode, codex_args) = runtime_mem_extract_mode_with_detail(&args.codex_args);
    let (_, codex_args) = extract_prodex_dry_run_flag(&codex_args);
    let (codex_args, include_code_review) =
        prepare_codex_launch_args(&codex_args, args.full_access);
    let model_provider_override = codex_cli_config_override_value(&codex_args, "model_provider");
    let request = RuntimeLaunchRequest {
        profile: args.profile.as_deref(),
        allow_auto_rotate: !args.no_auto_rotate,
        skip_quota_check: args.skip_quota_check,
        base_url: args.base_url.as_deref(),
        upstream_no_proxy: args.no_proxy,
        include_code_review,
        force_runtime_proxy: false,
        model_provider_override: model_provider_override.as_deref(),
    };
    print_runtime_launch_dry_run(
        "caveman",
        request,
        RuntimeLaunchDryRunChild::Caveman { codex_args },
    )
}

pub(crate) fn print_runtime_launch_dry_run(
    flow: &str,
    request: RuntimeLaunchRequest<'_>,
    child: RuntimeLaunchDryRunChild,
) -> Result<()> {
    let upstream_no_proxy = request.upstream_no_proxy;
    let prepared = prepare_runtime_launch_dry_run(request)?;
    let runtime_proxy = runtime_proxy_codex_endpoint(prepared.runtime_proxy.as_ref());
    let plan = prodex_runtime_launch::runtime_launch_dry_run_plan(
        codex_bin(),
        &prepared.codex_home,
        &prepared.paths.managed_profiles_root,
        runtime_proxy,
        upstream_no_proxy,
        SUPER_LOCAL_PROVIDER_ID,
        child,
    );
    let output = prodex_runtime_launch::runtime_launch_dry_run_report(
        flow,
        &prepared.codex_home,
        runtime_proxy,
        &plan,
    );
    print!("{output}");
    Ok(())
}

fn runtime_proxy_codex_endpoint(
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
) -> Option<prodex_runtime_launch::RuntimeProxyCodexEndpoint<'_>> {
    runtime_proxy.map(|proxy| prodex_runtime_launch::RuntimeProxyCodexEndpoint {
        listen_addr: proxy.listen_addr,
        openai_mount_path: &proxy.openai_mount_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_sandbox_removed_env_strips_inherited_codex_sandbox_vars() {
        let _env_guard = TestEnvVarGuard::lock();
        let _sandbox_guard = TestEnvVarGuard::set("CODEX_SANDBOX", "workspace-write");
        let _network_guard = TestEnvVarGuard::set("CODEX_SANDBOX_NETWORK_DISABLED", "1");
        let _custom_guard = TestEnvVarGuard::set("CODEX_SANDBOX_PROFILE", "danger-full-access");
        let _other_guard = TestEnvVarGuard::set("PRODEX_TEST_KEEP_ENV", "1");

        let removed = codex_sandbox_removed_env();

        assert!(removed.iter().any(|key| key == "CODEX_SANDBOX"));
        assert!(
            removed
                .iter()
                .any(|key| key == "CODEX_SANDBOX_NETWORK_DISABLED")
        );
        assert!(removed.iter().any(|key| key == "CODEX_SANDBOX_PROFILE"));
        assert!(!removed.iter().any(|key| key == "PRODEX_TEST_KEEP_ENV"));
    }

    #[test]
    fn codex_child_plan_applies_codex_sandbox_removed_env() {
        let _env_guard = TestEnvVarGuard::lock();
        let _custom_guard = TestEnvVarGuard::set("CODEX_SANDBOX_PROFILE", "danger-full-access");
        let _no_proxy_guard = TestEnvVarGuard::set("NO_PROXY", "example.com");
        let _lower_no_proxy_guard = TestEnvVarGuard::set("no_proxy", "internal.local");
        let codex_home = PathBuf::from("/tmp/prodex-codex-home");
        let args = vec![OsString::from("login")];

        let plan = codex_child_plan(codex_home.clone(), args.clone());

        assert_eq!(plan.binary, codex_bin());
        assert_eq!(plan.codex_home, codex_home);
        assert_eq!(plan.args, args);
        assert!(plan.removed_env.iter().any(|key| key == "CODEX_SANDBOX"));
        assert!(
            plan.removed_env
                .iter()
                .any(|key| key == "CODEX_SANDBOX_NETWORK_DISABLED")
        );
        assert!(
            plan.removed_env
                .iter()
                .any(|key| key == "CODEX_SANDBOX_PROFILE")
        );
        assert_eq!(
            plan.extra_env
                .iter()
                .find(|(key, _)| key == "NO_PROXY")
                .map(|(_, value)| value.to_string_lossy().into_owned()),
            Some("example.com,internal.local,127.0.0.1,localhost,::1".to_string())
        );
        assert_eq!(
            plan.extra_env
                .iter()
                .find(|(key, _)| key == "no_proxy")
                .map(|(_, value)| value.to_string_lossy().into_owned()),
            Some("example.com,internal.local,127.0.0.1,localhost,::1".to_string())
        );
    }

    #[test]
    fn local_proxy_bypass_env_deduplicates_existing_values() {
        let _env_guard = TestEnvVarGuard::lock();
        let _no_proxy_guard = TestEnvVarGuard::set("NO_PROXY", " example.com,127.0.0.1 ");
        let _lower_no_proxy_guard = TestEnvVarGuard::set("no_proxy", "LOCALHOST,internal.local");

        let env = local_proxy_bypass_env();

        assert_eq!(
            env,
            vec![
                (
                    "NO_PROXY",
                    OsString::from("example.com,127.0.0.1,LOCALHOST,internal.local,::1")
                ),
                (
                    "no_proxy",
                    OsString::from("example.com,127.0.0.1,LOCALHOST,internal.local,::1")
                )
            ]
        );
    }

    #[test]
    fn codex_child_plan_adds_local_provider_host_to_proxy_bypass_env() {
        let _env_guard = TestEnvVarGuard::lock();
        let _no_proxy_guard = TestEnvVarGuard::set("NO_PROXY", "example.com");
        let _lower_no_proxy_guard = TestEnvVarGuard::unset("no_proxy");
        let args = vec![
            OsString::from("-c"),
            OsString::from(format!(
                "model_providers.{SUPER_LOCAL_PROVIDER_ID}.base_url=\"http://host.docker.internal:11434/v1\""
            )),
        ];

        let plan = codex_child_plan(PathBuf::from("/tmp/prodex-codex-home"), args);

        assert_eq!(
            plan.extra_env
                .iter()
                .find(|(key, _)| key == "NO_PROXY")
                .map(|(_, value)| value.to_string_lossy().into_owned()),
            Some(
                "example.com,127.0.0.1,localhost,::1,host.docker.internal,host.docker.internal:11434"
                    .to_string()
            )
        );
        assert_eq!(
            plan.extra_env
                .iter()
                .find(|(key, _)| key == "no_proxy")
                .map(|(_, value)| value.to_string_lossy().into_owned()),
            Some(
                "example.com,127.0.0.1,localhost,::1,host.docker.internal,host.docker.internal:11434"
                    .to_string()
            )
        );
    }

    #[test]
    fn codex_child_plan_adds_runtime_proxy_ports_to_proxy_bypass_env() {
        let _env_guard = TestEnvVarGuard::lock();
        let _http_proxy_guard = TestEnvVarGuard::set("http_proxy", "http://127.0.0.1:1086");
        let _https_proxy_guard = TestEnvVarGuard::set("https_proxy", "http://127.0.0.1:1086");
        let _no_proxy_guard = TestEnvVarGuard::unset("NO_PROXY");
        let _lower_no_proxy_guard = TestEnvVarGuard::unset("no_proxy");
        let args = vec![
            OsString::from("-c"),
            OsString::from("chatgpt_base_url=\"http://127.0.0.1:64550/backend-api\""),
            OsString::from("-c"),
            OsString::from("openai_base_url=\"http://127.0.0.1:64550/backend-api/prodex\""),
        ];

        let plan = codex_child_plan(PathBuf::from("/tmp/prodex-codex-home"), args);

        assert_eq!(
            plan.extra_env
                .iter()
                .find(|(key, _)| key == "NO_PROXY")
                .map(|(_, value)| value.to_string_lossy().into_owned()),
            Some("127.0.0.1,localhost,::1,127.0.0.1:64550".to_string())
        );
        assert_eq!(
            plan.extra_env
                .iter()
                .find(|(key, _)| key == "no_proxy")
                .map(|(_, value)| value.to_string_lossy().into_owned()),
            Some("127.0.0.1,localhost,::1,127.0.0.1:64550".to_string())
        );
    }

    #[test]
    fn remove_upstream_proxy_env_preserves_local_proxy_bypass_env() {
        let _env_guard = TestEnvVarGuard::lock();
        let _http_proxy_guard = TestEnvVarGuard::set("HTTP_PROXY", "http://127.0.0.1:1086");
        let _https_proxy_guard = TestEnvVarGuard::set("https_proxy", "http://127.0.0.1:1086");
        let _no_proxy_guard = TestEnvVarGuard::set("NO_PROXY", "example.com");
        let _lower_no_proxy_guard = TestEnvVarGuard::unset("no_proxy");
        let mut plan = codex_child_plan(PathBuf::from("/tmp/prodex-codex-home"), vec![]);

        remove_upstream_proxy_env(&mut plan);

        assert!(plan.removed_env.iter().any(|key| key == "HTTP_PROXY"));
        assert!(plan.removed_env.iter().any(|key| key == "https_proxy"));
        assert_eq!(
            plan.extra_env
                .iter()
                .find(|(key, _)| key == "NO_PROXY")
                .map(|(_, value)| value.to_string_lossy().into_owned()),
            Some("example.com,127.0.0.1,localhost,::1".to_string())
        );
    }

    #[test]
    fn prepare_codex_launch_args_extracts_prodex_full_access_passthrough_marker() {
        let (args, include_code_review) = prepare_codex_launch_args(
            &[
                OsString::from("exec"),
                OsString::from("--full-access"),
                OsString::from("review"),
            ],
            false,
        );

        assert_eq!(
            args,
            vec![
                OsString::from("--dangerously-bypass-approvals-and-sandbox"),
                OsString::from("exec"),
                OsString::from("review"),
            ]
        );
        assert!(include_code_review);
    }

    #[test]
    fn prepare_codex_launch_args_full_access_keeps_resume_normalization_and_review_detection() {
        let (args, include_code_review) = prepare_codex_launch_args(
            &[
                OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
                OsString::from("review"),
            ],
            true,
        );

        assert_eq!(
            args,
            vec![
                OsString::from("--dangerously-bypass-approvals-and-sandbox"),
                OsString::from("resume"),
                OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
                OsString::from("review"),
            ]
        );
        assert!(include_code_review);
    }

    #[test]
    fn runtime_launch_dry_run_report_redacts_secret_env_and_args() {
        let codex_home = PathBuf::from("/tmp/prodex-home");
        let plan = RuntimeLaunchPlan::new(
            ChildProcessPlan::new(OsString::from("codex"), codex_home.clone())
                .with_args(vec![
                    OsString::from("-c"),
                    OsString::from("model=\"gpt-5.4\""),
                    OsString::from("--config=api_key=\"secret-value\""),
                    OsString::from("--api-key"),
                    OsString::from("opaque-cli-value"),
                    OsString::from("--header"),
                    OsString::from("Authorization: Bearer dry-run-bearer-secret-12345"),
                    OsString::from("sk-proj-dry-run-secret-123456789"),
                ])
                .with_extra_env(vec![
                    ("ANTHROPIC_AUTH_TOKEN", OsString::from("secret-value")),
                    (
                        "PRODEX_VISIBLE_BEARER",
                        OsString::from("Bearer dry-run-env-bearer-secret-12345"),
                    ),
                    ("PRODEX_VISIBLE", OsString::from("1")),
                ]),
        );
        let prepared = PreparedRuntimeLaunch {
            paths: AppPaths {
                root: PathBuf::from("/tmp/prodex"),
                state_file: PathBuf::from("/tmp/prodex/state.json"),
                managed_profiles_root: PathBuf::from("/tmp/prodex/profiles"),
                shared_codex_root: PathBuf::from("/tmp/prodex/shared"),
                legacy_shared_codex_root: PathBuf::from("/tmp/prodex/legacy-shared"),
            },
            codex_home,
            managed: false,
            runtime_proxy: None,
        };

        let report = prodex_runtime_launch::runtime_launch_dry_run_report(
            "run",
            &prepared.codex_home,
            runtime_proxy_codex_endpoint(prepared.runtime_proxy.as_ref()),
            &plan,
        );

        assert!(report.contains("Model: gpt-5.4"));
        assert!(report.contains("ANTHROPIC_AUTH_TOKEN=<redacted>"));
        assert!(report.contains("PRODEX_VISIBLE=1"));
        assert!(report.contains("Bearer <redacted>"));
        assert!(report.contains("sk-proj-<redacted>"));
        assert!(report.contains("<redacted>"));
        assert!(!report.contains("secret-value"));
        assert!(!report.contains("opaque-cli-value"));
        assert!(!report.contains("dry-run-bearer-secret-12345"));
        assert!(!report.contains("dry-run-secret-123456789"));
        assert!(!report.contains("dry-run-env-bearer-secret-12345"));
        assert!(report.contains("Codex/TUI not started"));
    }
}
