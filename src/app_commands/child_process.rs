use super::*;

const PRODEX_CODEX_FULL_ACCESS_ARG: &str = "--full-access";
const PRODEX_DRY_RUN_ARG: &str = "--dry-run";
const CODEX_BYPASS_APPROVALS_AND_SANDBOX_ARG: &str = "--dangerously-bypass-approvals-and-sandbox";

pub(crate) fn codex_child_plan(codex_home: PathBuf, args: Vec<OsString>) -> ChildProcessPlan {
    ChildProcessPlan::new(codex_bin(), codex_home)
        .with_args(args)
        .with_removed_env(codex_sandbox_removed_env())
}

pub(crate) fn codex_sandbox_removed_env() -> Vec<OsString> {
    let mut removed = BTreeSet::from([
        OsString::from("CODEX_SANDBOX"),
        OsString::from("CODEX_SANDBOX_NETWORK_DISABLED"),
    ]);
    removed.extend(env::vars_os().filter_map(|(key, _)| {
        key.to_str()
            .is_some_and(|value| value.starts_with("CODEX_SANDBOX"))
            .then_some(key)
    }));
    removed.into_iter().collect()
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

pub(crate) fn exit_with_status(status: ExitStatus) -> Result<()> {
    std::process::exit(status.code().unwrap_or(1));
}

pub(crate) fn prepare_codex_launch_args(
    codex_args: &[OsString],
    full_access_requested: bool,
) -> (Vec<OsString>, bool) {
    let (passthrough_full_access, codex_args) = extract_prodex_full_access_flag(codex_args);
    let codex_args = normalize_run_codex_args(&codex_args);
    let include_code_review = is_review_invocation(&codex_args);
    let codex_args = codex_launch_args_with_full_access(
        &codex_args,
        full_access_requested || passthrough_full_access,
    );
    (codex_args, include_code_review)
}

pub(crate) fn extract_prodex_dry_run_flag(codex_args: &[OsString]) -> (bool, Vec<OsString>) {
    let mut dry_run = false;
    let mut filtered = Vec::with_capacity(codex_args.len());
    for arg in codex_args {
        if arg == PRODEX_DRY_RUN_ARG {
            dry_run = true;
            continue;
        }
        filtered.push(arg.clone());
    }
    (dry_run, filtered)
}

pub(crate) fn prodex_dry_run_requested(codex_args: &[OsString]) -> bool {
    codex_args.iter().any(|arg| arg == PRODEX_DRY_RUN_ARG)
}

pub(crate) fn handle_caveman_dry_run(args: CavemanArgs) -> Result<()> {
    let (_mem_mode, codex_args) = runtime_mem_extract_mode(&args.codex_args);
    let (_, codex_args) = extract_prodex_dry_run_flag(&codex_args);
    let (codex_args, include_code_review) =
        prepare_codex_launch_args(&codex_args, args.full_access);
    let model_provider_override = codex_cli_config_override_value(&codex_args, "model_provider");
    let request = RuntimeLaunchRequest {
        profile: args.profile.as_deref(),
        allow_auto_rotate: !args.no_auto_rotate,
        skip_quota_check: args.skip_quota_check,
        base_url: args.base_url.as_deref(),
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

pub(crate) enum RuntimeLaunchDryRunChild {
    Codex { codex_args: Vec<OsString> },
    Caveman { codex_args: Vec<OsString> },
}

pub(crate) fn print_runtime_launch_dry_run(
    flow: &str,
    request: RuntimeLaunchRequest<'_>,
    child: RuntimeLaunchDryRunChild,
) -> Result<()> {
    let prepared = prepare_runtime_launch_dry_run(request)?;
    let runtime_proxy = prepared.runtime_proxy.as_ref();
    let plan = match child {
        RuntimeLaunchDryRunChild::Codex { codex_args } => {
            let runtime_args = runtime_proxy_codex_passthrough_args(runtime_proxy, &codex_args);
            RuntimeLaunchPlan::new(codex_child_plan(prepared.codex_home.clone(), runtime_args))
        }
        RuntimeLaunchDryRunChild::Caveman { codex_args } => {
            let runtime_args = runtime_proxy_codex_passthrough_args(runtime_proxy, &codex_args);
            let caveman_home =
                dry_run_caveman_home_placeholder(&prepared.paths, &prepared.codex_home);
            RuntimeLaunchPlan::new(codex_child_plan(caveman_home.clone(), runtime_args))
                .with_cleanup_path(caveman_home)
        }
    };
    let output = runtime_launch_dry_run_report(flow, &prepared, &plan);
    print!("{output}");
    Ok(())
}

fn dry_run_caveman_home_placeholder(paths: &AppPaths, base_codex_home: &Path) -> PathBuf {
    paths.managed_profiles_root.join(format!(
        ".caveman-dry-run-from-{}",
        base_codex_home
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("profile")
    ))
}

pub(crate) fn runtime_launch_dry_run_report(
    flow: &str,
    prepared: &PreparedRuntimeLaunch,
    plan: &RuntimeLaunchPlan,
) -> String {
    let child = &plan.child;
    let provider = dry_run_config_value(&child.args, &prepared.codex_home, "model_provider")
        .unwrap_or_else(|| "openai".to_string());
    let model = dry_run_config_value(&child.args, &prepared.codex_home, "model")
        .unwrap_or_else(|| "(codex default)".to_string());
    let mut output = String::new();
    output.push_str("Prodex dry run: launch diagnostics\n");
    output.push_str(&format!("Flow: {flow}\n"));
    output.push_str(&format!("Binary: {}\n", dry_run_os(&child.binary)));
    output.push_str(&format!("Provider: {provider}\n"));
    output.push_str(&format!("Model: {model}\n"));
    output.push_str(&format!("CODEX_HOME: {}\n", child.codex_home.display()));
    output.push_str(&format!(
        "Runtime proxy: {}\n",
        prepared
            .runtime_proxy
            .as_ref()
            .map(|proxy| {
                if proxy.listen_addr.port() == 0 {
                    format!("would be enabled with mount {}", proxy.openai_mount_path)
                } else {
                    format!(
                        "enabled at http://{}{}",
                        proxy.listen_addr, proxy.openai_mount_path
                    )
                }
            })
            .unwrap_or_else(|| "disabled".to_string())
    ));
    output.push_str("Args:\n");
    if child.args.is_empty() {
        output.push_str("  (none)\n");
    } else {
        for arg in &child.args {
            output.push_str(&format!("  {}\n", dry_run_redacted_arg(arg)));
        }
    }
    output.push_str("Env:\n");
    output.push_str(&format!("  CODEX_HOME={}\n", child.codex_home.display()));
    for (key, value) in &child.extra_env {
        output.push_str(&format!(
            "  {}={}\n",
            dry_run_os(key),
            dry_run_redacted_env_value(key, value)
        ));
    }
    if !child.removed_env.is_empty() {
        output.push_str("Removed env:\n");
        for key in &child.removed_env {
            output.push_str(&format!("  {}\n", dry_run_os(key)));
        }
    }
    output.push_str("Codex/TUI not started because --dry-run was set.\n");
    output
}

fn dry_run_config_value(args: &[OsString], codex_home: &Path, key: &str) -> Option<String> {
    codex_cli_config_override_value(args, key).or_else(|| codex_config_value(codex_home, key))
}

fn dry_run_redacted_env_value(key: &OsString, value: &OsString) -> String {
    if dry_run_key_looks_secret(key) {
        "<redacted>".to_string()
    } else {
        dry_run_os(value)
    }
}

fn dry_run_redacted_arg(value: &OsString) -> String {
    if dry_run_value_looks_secret(&value.to_string_lossy()) {
        "<redacted>".to_string()
    } else {
        dry_run_os(value)
    }
}

fn dry_run_os(value: &OsString) -> String {
    let value = value.to_string_lossy();
    if value.is_empty() {
        return "''".to_string();
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':' | '='))
    {
        return value.into_owned();
    }
    format!("{value:?}")
}

fn dry_run_key_looks_secret(key: &OsString) -> bool {
    dry_run_value_looks_secret(&key.to_string_lossy())
}

fn dry_run_value_looks_secret(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "access_token",
        "api_key",
        "apikey",
        "auth",
        "authorization",
        "bearer",
        "password",
        "secret",
        "token",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

pub(crate) fn is_review_invocation(args: &[OsString]) -> bool {
    args.iter().any(|arg| arg == "review")
}

fn extract_prodex_full_access_flag(codex_args: &[OsString]) -> (bool, Vec<OsString>) {
    let mut full_access = false;
    let mut filtered = Vec::with_capacity(codex_args.len());
    for arg in codex_args {
        if arg == PRODEX_CODEX_FULL_ACCESS_ARG {
            full_access = true;
            continue;
        }
        filtered.push(arg.clone());
    }
    (full_access, filtered)
}

fn codex_launch_args_with_full_access(codex_args: &[OsString], full_access: bool) -> Vec<OsString> {
    if !full_access {
        return codex_args.to_vec();
    }

    let mut args = Vec::with_capacity(codex_args.len() + 1);
    args.push(OsString::from(CODEX_BYPASS_APPROVALS_AND_SANDBOX_ARG));
    args.extend(codex_args.iter().cloned());
    args
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
                ])
                .with_extra_env(vec![
                    ("ANTHROPIC_AUTH_TOKEN", OsString::from("secret-value")),
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

        let report = runtime_launch_dry_run_report("run", &prepared, &plan);

        assert!(report.contains("Model: gpt-5.4"));
        assert!(report.contains("ANTHROPIC_AUTH_TOKEN=<redacted>"));
        assert!(report.contains("<redacted>"));
        assert!(!report.contains("secret-value"));
        assert!(report.contains("Codex/TUI not started"));
    }
}
