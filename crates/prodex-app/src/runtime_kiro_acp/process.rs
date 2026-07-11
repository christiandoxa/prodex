use super::{
    RuntimeKiroAcpClientInfo, RuntimeKiroAcpEnvelope, RuntimeKiroAcpInitializeResult,
    RuntimeKiroAcpNewSessionResult, runtime_kiro_acp_initialize_request,
    runtime_kiro_acp_session_new_request, runtime_kiro_acp_session_prompt_request,
};
use anyhow::{Context, Result, bail};
use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuntimeKiroAcpBootstrapResult {
    pub(crate) initialize: RuntimeKiroAcpInitializeResult,
    pub(crate) session: RuntimeKiroAcpNewSessionResult,
    pub(crate) notifications: Vec<RuntimeKiroAcpEnvelope>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuntimeKiroAcpPromptTurnResult {
    pub(crate) initialize: RuntimeKiroAcpInitializeResult,
    pub(crate) session: RuntimeKiroAcpNewSessionResult,
    pub(crate) prompt_response: RuntimeKiroAcpEnvelope,
    pub(crate) notifications: Vec<RuntimeKiroAcpEnvelope>,
}

pub(crate) fn runtime_kiro_acp_bootstrap(
    cwd: &Path,
    extra_env: &[(OsString, OsString)],
) -> Result<RuntimeKiroAcpBootstrapResult> {
    runtime_kiro_acp_bootstrap_with_command(crate::kiro_bin().as_os_str(), cwd, extra_env)
}

pub(crate) fn runtime_kiro_acp_bootstrap_with_command(
    command: &OsStr,
    cwd: &Path,
    extra_env: &[(OsString, OsString)],
) -> Result<RuntimeKiroAcpBootstrapResult> {
    let mut child = Command::new(command)
        .arg("acp")
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .envs(extra_env.iter().cloned())
        .spawn()
        .with_context(|| {
            format!(
                "failed to start Kiro ACP agent {}",
                command.to_string_lossy()
            )
        })?;
    let result = runtime_kiro_acp_bootstrap_child(&mut child, cwd);
    let _ = child.kill();
    let _ = child.wait();
    result
}

pub(crate) fn runtime_kiro_acp_prompt_turn(
    cwd: &Path,
    extra_env: &[(OsString, OsString)],
    prompt: &str,
) -> Result<RuntimeKiroAcpPromptTurnResult> {
    runtime_kiro_acp_prompt_turn_with_command(crate::kiro_bin().as_os_str(), cwd, extra_env, prompt)
}

pub(crate) fn runtime_kiro_acp_prompt_turn_with_command(
    command: &OsStr,
    cwd: &Path,
    extra_env: &[(OsString, OsString)],
    prompt: &str,
) -> Result<RuntimeKiroAcpPromptTurnResult> {
    let mut child = Command::new(command)
        .arg("acp")
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .envs(extra_env.iter().cloned())
        .spawn()
        .with_context(|| {
            format!(
                "failed to start Kiro ACP agent {}",
                command.to_string_lossy()
            )
        })?;
    let result = runtime_kiro_acp_prompt_turn_child(&mut child, cwd, prompt);
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn runtime_kiro_acp_bootstrap_child(
    child: &mut std::process::Child,
    cwd: &Path,
) -> Result<RuntimeKiroAcpBootstrapResult> {
    let mut stdin = BufWriter::new(
        child
            .stdin
            .take()
            .context("failed to capture Kiro ACP stdin")?,
    );
    writeln!(
        stdin,
        "{}",
        runtime_kiro_acp_initialize_request(
            0,
            RuntimeKiroAcpClientInfo {
                name: "prodex",
                title: "Prodex",
                version: env!("CARGO_PKG_VERSION"),
            },
        )
    )
    .context("failed to write Kiro ACP initialize request")?;
    writeln!(stdin, "{}", runtime_kiro_acp_session_new_request(1, cwd))
        .context("failed to write Kiro ACP session/new request")?;
    stdin
        .flush()
        .context("failed to flush Kiro ACP bootstrap requests")?;
    drop(stdin);

    let stdout = child
        .stdout
        .take()
        .context("failed to capture Kiro ACP stdout")?;
    let mut stderr = child
        .stderr
        .take()
        .context("failed to capture Kiro ACP stderr")?;
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let mut initialize = None;
    let mut session = None;
    let mut notifications = Vec::new();
    while reader
        .read_line(&mut line)
        .context("failed to read Kiro ACP stdout")?
        > 0
    {
        let current = line.trim();
        if current.is_empty() {
            line.clear();
            continue;
        }
        let envelope = RuntimeKiroAcpEnvelope::parse(current)?;
        if let Some(error) = &envelope.error {
            if matches!(envelope.id, Some(0 | 1)) {
                let stderr_text = runtime_kiro_acp_read_stderr(&mut stderr);
                bail!(
                    "Kiro ACP bootstrap failed for request {:?}: {}{}",
                    envelope.id,
                    error.message,
                    runtime_kiro_acp_stderr_suffix(&stderr_text)
                );
            }
            notifications.push(envelope);
            line.clear();
            continue;
        }
        match envelope.id {
            Some(0) => initialize = Some(envelope.parse_initialize_result()?),
            Some(1) => session = Some(envelope.parse_session_new_result()?),
            _ => notifications.push(envelope),
        }
        if initialize.is_some() && session.is_some() {
            break;
        }
        line.clear();
    }

    let stderr_text = runtime_kiro_acp_read_stderr(&mut stderr);
    let initialize = initialize.with_context(|| {
        format!(
            "Kiro ACP bootstrap did not return initialize result{}",
            runtime_kiro_acp_stderr_suffix(&stderr_text)
        )
    })?;
    let session = session.with_context(|| {
        format!(
            "Kiro ACP bootstrap did not return session/new result{}",
            runtime_kiro_acp_stderr_suffix(&stderr_text)
        )
    })?;
    Ok(RuntimeKiroAcpBootstrapResult {
        initialize,
        session,
        notifications,
    })
}

fn runtime_kiro_acp_prompt_turn_child(
    child: &mut std::process::Child,
    cwd: &Path,
    prompt: &str,
) -> Result<RuntimeKiroAcpPromptTurnResult> {
    let mut stdin = BufWriter::new(
        child
            .stdin
            .take()
            .context("failed to capture Kiro ACP stdin")?,
    );
    writeln!(
        stdin,
        "{}",
        runtime_kiro_acp_initialize_request(
            0,
            RuntimeKiroAcpClientInfo {
                name: "prodex",
                title: "Prodex",
                version: env!("CARGO_PKG_VERSION"),
            },
        )
    )
    .context("failed to write Kiro ACP initialize request")?;
    writeln!(stdin, "{}", runtime_kiro_acp_session_new_request(1, cwd))
        .context("failed to write Kiro ACP session/new request")?;
    stdin
        .flush()
        .context("failed to flush initial Kiro ACP prompt-turn requests")?;

    let stdout = child
        .stdout
        .take()
        .context("failed to capture Kiro ACP stdout")?;
    let mut stderr = child
        .stderr
        .take()
        .context("failed to capture Kiro ACP stderr")?;
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let mut initialize = None;
    let mut session = None;
    let mut prompt_response = None;
    let mut notifications = Vec::new();
    let mut prompt_sent = false;
    while reader
        .read_line(&mut line)
        .context("failed to read Kiro ACP stdout")?
        > 0
    {
        let current = line.trim();
        if current.is_empty() {
            line.clear();
            continue;
        }
        let envelope = RuntimeKiroAcpEnvelope::parse(current)?;
        if !prompt_sent && matches!(envelope.id, Some(1)) && envelope.error.is_none() {
            let parsed_session = envelope.parse_session_new_result()?;
            writeln!(
                stdin,
                "{}",
                runtime_kiro_acp_session_prompt_request(2, &parsed_session.session_id, prompt)
            )
            .context("failed to write Kiro ACP session/prompt request")?;
            stdin
                .flush()
                .context("failed to flush Kiro ACP session/prompt request")?;
            prompt_sent = true;
            session = Some(parsed_session);
            line.clear();
            continue;
        }
        match envelope.id {
            Some(0) if envelope.error.is_none() => {
                initialize = Some(envelope.parse_initialize_result()?)
            }
            Some(1) if envelope.error.is_none() => {
                session = Some(envelope.parse_session_new_result()?)
            }
            Some(2) => {
                prompt_response = Some(envelope);
                break;
            }
            _ => notifications.push(envelope),
        }
        line.clear();
    }
    drop(stdin);

    let stderr_text = runtime_kiro_acp_read_stderr(&mut stderr);
    let initialize = initialize.with_context(|| {
        format!(
            "Kiro ACP prompt turn did not return initialize result{}",
            runtime_kiro_acp_stderr_suffix(&stderr_text)
        )
    })?;
    let session = session.with_context(|| {
        format!(
            "Kiro ACP prompt turn did not return session/new result{}",
            runtime_kiro_acp_stderr_suffix(&stderr_text)
        )
    })?;
    let prompt_response = prompt_response.with_context(|| {
        format!(
            "Kiro ACP prompt turn did not return session/prompt response{}",
            runtime_kiro_acp_stderr_suffix(&stderr_text)
        )
    })?;
    Ok(RuntimeKiroAcpPromptTurnResult {
        initialize,
        session,
        prompt_response,
        notifications,
    })
}

fn runtime_kiro_acp_read_stderr(stderr: &mut impl Read) -> String {
    let mut stderr_text = String::new();
    let _ = stderr.read_to_string(&mut stderr_text);
    stderr_text
}

fn runtime_kiro_acp_stderr_suffix(stderr: &str) -> String {
    let stderr = stderr.trim();
    if stderr.is_empty() {
        String::new()
    } else {
        format!("; stderr: {stderr}")
    }
}
