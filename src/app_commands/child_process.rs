use super::*;

const PRODEX_CODEX_FULL_ACCESS_ARG: &str = "--full-access";
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
}
