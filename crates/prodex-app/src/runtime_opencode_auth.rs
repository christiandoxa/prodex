use anyhow::{Context, Result, ensure};
use prodex_cli::{SUPER_OPENCODE_PROVIDER_ID, SuperExternalProvider};
use prodex_runtime_launch::ChildProcessPlan;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

const CONSOLE_CLI: &str = "opencode-console";
pub(crate) const OPENCODE_API_KEY: &str = "OPENCODE_API_KEY";
const INSTALL_GUIDANCE: &str =
    "OpenCode Console CLI is required. Install @opencode/console and put opencode-console on PATH.";

fn console_cli() -> Option<PathBuf> {
    // npm also installs an extensionless shell script on Windows.
    #[cfg(windows)]
    for binary in ["opencode-console.exe", "opencode-console.cmd"] {
        if let Some(path) = prodex_core::resolve_binary_path(&OsString::from(binary)) {
            return Some(path);
        }
    }
    prodex_core::resolve_binary_path(&OsString::from(CONSOLE_CLI))
}

pub(crate) fn run_opencode_login() -> Result<ExitStatus> {
    let cli = console_cli().context(INSTALL_GUIDANCE)?;
    let mut command = Command::new(cli);
    command.arg("login");
    for key in crate::PROVIDER_SECRET_ENV_KEYS {
        command.env_remove(key);
    }
    command
        .status()
        .context("failed to start OpenCode Console sign-in")
}

pub(crate) fn configure_opencode_codex_args(
    args: &mut Vec<OsString>,
    explicit_key: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    let env_key = env::var_os(OPENCODE_API_KEY);
    let auth = if let Some(key) = explicit_key.map(OsString::from).or(env_key) {
        ensure!(
            key.to_str().is_some_and(|key| {
                !key.is_empty() && key.bytes().all(|byte| byte.is_ascii_graphic())
            }),
            "OpenCode API key must be nonempty and contain no whitespace or control bytes"
        );
        format!("env_key = \"{OPENCODE_API_KEY}\"")
    } else {
        let cli = if dry_run {
            PathBuf::from(CONSOLE_CLI)
        } else {
            console_cli().context(INSTALL_GUIDANCE)?
        };
        let command = cli
            .to_str()
            .context("OpenCode Console CLI path must be UTF-8")?;
        let command = crate::runtime_catalog_config::toml_string_literal(command);
        format!(
            "auth = {{ command = {command}, args = [\"token\"], refresh_interval_ms = 240000, timeout_ms = 15000 }}"
        )
    };
    let base_url = SuperExternalProvider::OpenCode.default_base_url();
    // Replace the provider table so saved or forwarded config cannot redirect its credentials.
    let provider = format!(
        "model_providers.{SUPER_OPENCODE_PROVIDER_ID}={{ name = \"OpenCode Go\", base_url = \"{base_url}\", wire_api = \"responses\", capabilities = {{ remote_compaction = \"unsupported\" }}, {auth} }}"
    );
    let insert_at = args
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or(args.len());
    args.splice(insert_at..insert_at, ["-c".into(), provider.into()]);
    Ok(())
}

pub(crate) fn apply_opencode_key_to_child(child: &mut ChildProcessPlan, key: Option<&str>) {
    if let Some(key) = key
        .map(OsString::from)
        .or_else(|| env::var_os(OPENCODE_API_KEY))
    {
        child.extra_env.push((OPENCODE_API_KEY.into(), key));
    }
}
