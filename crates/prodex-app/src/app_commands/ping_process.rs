use super::super::{
    ChildProcessPlan, ObservedCommandOutput, codex_child_plan,
    command_output_with_timeout_matching_stdout_line, prepare_codex_launch_args,
    profile_openai_compatible_codex_args, remove_provider_secret_env, remove_upstream_proxy_env,
    runtime_launch_openai_model_context_codex_args,
};
use super::{
    PING_OUTPUT_MAX_BYTES, PING_PROMPT, PING_TIMEOUT, PingProbeOptions, PingStatus, PingTarget,
    ping_status_from_mojo,
};
use anyhow::{Context, Result, bail};
use redaction::redaction_redact_secret_like_text;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(super) fn classify_ping_process_error(error: &anyhow::Error) -> (PingStatus, String) {
    // anyhow's native error chain stays in Rust; Mojo receives acquired text.
    let message = format!("{error:#}");
    let redacted = redaction_redact_secret_like_text(&message);
    let (status, detail) = prodex_mojo_core::rich::ping_process_error(&message, &redacted)
        .expect("Mojo ping process diagnostic returned invalid output");
    (ping_status_from_mojo(status), detail)
}

pub(super) fn ping_command_args(options: &PingProbeOptions) -> Vec<OsString> {
    let mut command_args = vec![OsString::from("exec")];
    command_args.extend([
        OsString::from("--sandbox"),
        OsString::from("read-only"),
        OsString::from("--ephemeral"),
        OsString::from("--ignore-user-config"),
        OsString::from("--ignore-rules"),
        OsString::from("--skip-git-repo-check"),
    ]);
    command_args.extend([
        OsString::from("-c"),
        OsString::from("model_provider=\"openai\""),
    ]);
    if let Some(base_url) = options.base_url.as_deref() {
        command_args.extend([
            OsString::from("-c"),
            OsString::from(format!(
                "chatgpt_base_url={}",
                crate::runtime_catalog_config::toml_string_literal(base_url)
            )),
        ]);
    }
    if let Some(model) = options.model.as_deref() {
        command_args.extend([OsString::from("--model"), OsString::from(model)]);
    }
    if let Some(effort) = options.effort.as_deref() {
        command_args.extend([
            OsString::from("-c"),
            OsString::from(format!("model_reasoning_effort={effort}")),
        ]);
    }
    command_args.extend([
        OsString::from("--json"),
        OsString::from("--color"),
        OsString::from("never"),
        OsString::from(PING_PROMPT),
    ]);
    command_args
}

fn ping_child_plan(target: &PingTarget, options: &PingProbeOptions) -> Result<ChildProcessPlan> {
    let (args, _) = prepare_codex_launch_args(&ping_command_args(options), false);
    let args = runtime_launch_openai_model_context_codex_args(&target.codex_home, &args)?;
    let args = profile_openai_compatible_codex_args(&target.codex_home, &args)?;
    let mut plan = codex_child_plan(target.codex_home.clone(), args);
    remove_provider_secret_env(&mut plan);
    if options.no_proxy {
        remove_upstream_proxy_env(&mut plan);
    }
    Ok(plan)
}

pub(super) fn run_ping_command(
    target: &PingTarget,
    options: &PingProbeOptions,
) -> Result<ObservedCommandOutput> {
    let plan = ping_child_plan(target, options)?;
    run_ping_child(&plan, PING_TIMEOUT)
}

pub(super) fn run_ping_child(
    plan: &ChildProcessPlan,
    timeout: Duration,
) -> Result<ObservedCommandOutput> {
    let cwd = create_ping_cwd()?;
    run_ping_child_in_cwd(plan, timeout, &cwd)
}

fn run_ping_child_in_cwd(
    plan: &ChildProcessPlan,
    timeout: Duration,
    cwd: &Path,
) -> Result<ObservedCommandOutput> {
    crate::validate_selected_codex_binary(&plan.binary)?;
    let result = {
        let mut command = Command::new(&plan.binary);
        command
            .args(&plan.args)
            .env("CODEX_HOME", &plan.codex_home)
            .current_dir(cwd);
        for key in &plan.removed_env {
            command.env_remove(key);
        }
        for (key, value) in &plan.extra_env {
            command.env(key, value);
        }
        command_output_with_timeout_matching_stdout_line(
            &mut command,
            timeout,
            PING_OUTPUT_MAX_BYTES,
            "OpenAI application ping",
            Some(ping_stdout_line_is_model_response),
        )
    };
    let cleanup = cleanup_ping_cwd(cwd);
    match (result, cleanup) {
        (Ok(output), Ok(())) => Ok(output),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error).context("failed to clean the diagnostic directory"),
        (Err(error), Err(cleanup_error)) => Err(error).context(format!(
            "failed to clean the diagnostic directory: {cleanup_error}"
        )),
    }
}

#[cfg(test)]
pub(super) fn run_ping_child_in_test_cwd(
    plan: &ChildProcessPlan,
    timeout: Duration,
    cwd: &Path,
) -> Result<ObservedCommandOutput> {
    run_ping_child_in_cwd(plan, timeout, cwd)
}

fn ping_stdout_line_is_model_response(line: &[u8]) -> bool {
    let Ok(event) = serde_json::from_slice::<serde_json::Value>(line) else {
        return false;
    };
    event.get("type").and_then(serde_json::Value::as_str) == Some("item.completed")
        && event
            .pointer("/item/type")
            .and_then(serde_json::Value::as_str)
            == Some("agent_message")
        && event
            .pointer("/item/text")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|text| !text.trim().is_empty())
}

pub(super) fn ping_output_failure_detail(base: &str, output: &Output) -> String {
    use prodex_mojo_core::rich::PingChildExit;
    let exit = if output.status.success() {
        PingChildExit::Success
    } else {
        PingChildExit::Failed(crate::child_exit_code(&output.status))
    };
    let stderr = redaction_redact_secret_like_text(&String::from_utf8_lossy(&output.stderr));
    prodex_mojo_core::rich::ping_output_failure_detail(base, exit, &stderr)
        .expect("Mojo ping output diagnostic returned invalid output")
}

pub(super) fn bounded_ping_detail(text: &str) -> String {
    let text = redaction_redact_secret_like_text(text);
    prodex_mojo_core::rich::ping_bounded_detail(&text)
        .expect("Mojo bounded ping diagnostic returned invalid output")
}

fn cleanup_ping_cwd(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            match fs::remove_dir_all(path) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(_) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(error) => return Err(error),
            }
        }
    }
    #[cfg(not(windows))]
    {
        fs::remove_dir_all(path)
    }
}

fn create_ping_cwd() -> Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for attempt in 0..16u32 {
        let path = std::env::temp_dir().join(format!(
            "prodex-ping-{}-{stamp}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).context("failed to create diagnostic directory"),
        }
    }
    bail!("could not allocate a private diagnostic directory")
}
