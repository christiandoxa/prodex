use super::super::{
    ChildProcessPlan, codex_child_plan, command_output_with_timeout, prepare_codex_launch_args,
    profile_openai_compatible_codex_args, remove_provider_secret_env, remove_upstream_proxy_env,
    runtime_launch_openai_spark_context_codex_args,
};
use super::{
    PING_OUTPUT_MAX_BYTES, PING_PROMPT, PING_TIMEOUT, PingProbeOptions, PingStatus, PingTarget,
    classify_failure_text,
};
use anyhow::{Context, Result, bail};
use redaction::redaction_redact_secret_like_text;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(super) const PING_ERROR_DETAIL_MAX_BYTES: usize = 4096;

pub(super) fn classify_ping_process_error(error: &anyhow::Error) -> (PingStatus, String) {
    let message = format!("{error:#}");
    let classified = classify_failure_text(&message);
    let (status, detail) = if classified.0 != PingStatus::ProcessFailed {
        classified
    } else if message.contains("timed out") {
        (PingStatus::Timeout, "OpenAI application ping timed out")
    } else if message.contains("failed to start") {
        (
            PingStatus::SpawnFailed,
            "OpenAI application ping could not start",
        )
    } else if message.contains("clean the diagnostic directory") {
        (
            PingStatus::ProcessFailed,
            "OpenAI application ping cleanup failed",
        )
    } else {
        (
            PingStatus::ProcessFailed,
            "OpenAI application ping process failed",
        )
    };
    let message = bounded_ping_detail(&message);
    (
        status,
        if message.is_empty() || message == detail {
            detail.to_string()
        } else {
            format!("{detail}: {message}")
        },
    )
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
    let args = runtime_launch_openai_spark_context_codex_args(&target.codex_home, &args)?;
    let args = profile_openai_compatible_codex_args(&target.codex_home, &args)?;
    let mut plan = codex_child_plan(target.codex_home.clone(), args);
    remove_provider_secret_env(&mut plan);
    if options.no_proxy {
        remove_upstream_proxy_env(&mut plan);
    }
    Ok(plan)
}

pub(super) fn run_ping_command(target: &PingTarget, options: &PingProbeOptions) -> Result<Output> {
    let plan = ping_child_plan(target, options)?;
    run_ping_child(&plan, PING_TIMEOUT)
}

pub(super) fn run_ping_child(plan: &ChildProcessPlan, timeout: Duration) -> Result<Output> {
    let cwd = create_ping_cwd()?;
    let result = {
        let mut command = Command::new(&plan.binary);
        command
            .args(&plan.args)
            .env("CODEX_HOME", &plan.codex_home)
            .current_dir(&cwd);
        for key in &plan.removed_env {
            command.env_remove(key);
        }
        for (key, value) in &plan.extra_env {
            command.env(key, value);
        }
        command_output_with_timeout(
            &mut command,
            timeout,
            PING_OUTPUT_MAX_BYTES,
            "OpenAI application ping",
        )
    };
    let cleanup = cleanup_ping_cwd(&cwd);
    match (result, cleanup) {
        (Ok(output), Ok(())) => Ok(output),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error).context("failed to clean the diagnostic directory"),
        (Err(error), Err(cleanup_error)) => Err(error).context(format!(
            "failed to clean the diagnostic directory: {cleanup_error}"
        )),
    }
}

pub(super) fn ping_output_failure_detail(base: &str, output: &Output) -> String {
    let mut detail = base.to_string();
    if !output.status.success() {
        detail.push_str(&format!(
            " (exit code {})",
            crate::child_exit_code(&output.status)
        ));
    }
    let stderr = bounded_ping_detail(&String::from_utf8_lossy(&output.stderr));
    if !stderr.is_empty() {
        detail.push_str(": stderr: ");
        detail.push_str(&stderr);
    }
    detail
}

fn bounded_ping_detail(text: &str) -> String {
    let text = redaction_redact_secret_like_text(text)
        .replace(['\r', '\n'], " ")
        .trim()
        .to_string();
    if text.len() <= PING_ERROR_DETAIL_MAX_BYTES {
        return text;
    }
    let end = text
        .char_indices()
        .take_while(|(index, _)| *index <= PING_ERROR_DETAIL_MAX_BYTES)
        .map(|(index, _)| index)
        .last()
        .unwrap_or_default()
        .min(PING_ERROR_DETAIL_MAX_BYTES);
    text[..end].to_string()
}

fn cleanup_ping_cwd(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            match fs::remove_dir_all(path) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(error) if std::time::Instant::now() < deadline => {
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
