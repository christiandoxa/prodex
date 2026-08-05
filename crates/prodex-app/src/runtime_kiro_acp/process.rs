use super::{
    RuntimeKiroAcpClientInfo, RuntimeKiroAcpEnvelope, RuntimeKiroAcpInitializeResult,
    RuntimeKiroAcpNewSessionResult, runtime_kiro_acp_initialize_request,
    runtime_kiro_acp_session_new_request, runtime_kiro_acp_session_prompt_request,
};
use crate::RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES;
use anyhow::{Context, Result, bail};
use std::ffi::{OsStr, OsString};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const RUNTIME_KIRO_ACP_BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(5);
const RUNTIME_KIRO_ACP_PROMPT_TURN_TIMEOUT: Duration = Duration::from_secs(120);

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

pub(crate) fn runtime_kiro_acp_bootstrap_with_command(
    command: &OsStr,
    cwd: &Path,
    extra_env: &[(OsString, OsString)],
) -> Result<RuntimeKiroAcpBootstrapResult> {
    runtime_kiro_acp_bootstrap_with_command_and_timeout(
        command,
        cwd,
        extra_env,
        RUNTIME_KIRO_ACP_BOOTSTRAP_TIMEOUT,
    )
}

pub(crate) fn runtime_kiro_acp_bootstrap_with_command_and_timeout(
    command: &OsStr,
    cwd: &Path,
    extra_env: &[(OsString, OsString)],
    timeout: Duration,
) -> Result<RuntimeKiroAcpBootstrapResult> {
    let mut child = Command::new(command)
        .arg("acp")
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .envs(extra_env.iter().cloned())
        .spawn()
        .with_context(|| {
            format!(
                "failed to start Kiro ACP agent {}",
                command.to_string_lossy()
            )
        })?;
    let result = runtime_kiro_acp_bootstrap_child(&mut child, cwd, timeout);
    let _ = child.kill();
    let _ = child.wait();
    result
}

#[cfg(test)]
pub(crate) fn runtime_kiro_acp_prompt_turn_with_command(
    command: &OsStr,
    cwd: &Path,
    extra_env: &[(OsString, OsString)],
    prompt: &str,
) -> Result<RuntimeKiroAcpPromptTurnResult> {
    runtime_kiro_acp_prompt_turn_with_command_and_options(
        command, cwd, extra_env, None, None, prompt,
    )
}

pub(crate) fn runtime_kiro_acp_prompt_turn_with_command_and_options(
    command: &OsStr,
    cwd: &Path,
    extra_env: &[(OsString, OsString)],
    model: Option<&str>,
    effort: Option<&str>,
    prompt: &str,
) -> Result<RuntimeKiroAcpPromptTurnResult> {
    runtime_kiro_acp_prompt_turn_with_command_and_options_and_timeout(
        command,
        cwd,
        extra_env,
        model,
        effort,
        prompt,
        RUNTIME_KIRO_ACP_PROMPT_TURN_TIMEOUT,
    )
}

pub(crate) fn runtime_kiro_acp_prompt_turn_with_command_and_options_and_timeout(
    command: &OsStr,
    cwd: &Path,
    extra_env: &[(OsString, OsString)],
    model: Option<&str>,
    effort: Option<&str>,
    prompt: &str,
    timeout: Duration,
) -> Result<RuntimeKiroAcpPromptTurnResult> {
    let mut command = Command::new(command);
    command.arg("acp");
    if let Some(model) = model.map(str::trim).filter(|model| !model.is_empty()) {
        command.arg("--model").arg(model);
    }
    if let Some(effort) = effort.map(str::trim).filter(|effort| !effort.is_empty()) {
        command.arg("--effort").arg(effort);
    }
    let mut child = command
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .envs(extra_env.iter().cloned())
        .spawn()
        .with_context(|| {
            format!(
                "failed to start Kiro ACP agent {}",
                command.get_program().to_string_lossy()
            )
        })?;
    let result = runtime_kiro_acp_prompt_turn_child(&mut child, cwd, prompt, timeout);
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn runtime_kiro_acp_bootstrap_child(
    child: &mut std::process::Child,
    cwd: &Path,
    timeout: Duration,
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
    let stdout = child
        .stdout
        .take()
        .context("failed to capture Kiro ACP stdout")?;
    let lines = runtime_kiro_acp_line_receiver(stdout);
    let deadline = Instant::now() + timeout;
    let mut initialize = None;
    let mut session = None;
    let mut notifications = Vec::new();
    loop {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            bail!("Kiro ACP bootstrap timed out");
        };
        let line = match lines.recv_timeout(remaining) {
            Ok(Ok(line)) => line,
            Ok(Err(error)) => return Err(error).context("failed to read Kiro ACP stdout"),
            Err(mpsc::RecvTimeoutError::Timeout) => bail!("Kiro ACP bootstrap timed out"),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        let current = line.trim();
        if current.is_empty() {
            continue;
        }
        let envelope = RuntimeKiroAcpEnvelope::parse(current)?;
        if runtime_kiro_acp_reject_unsupported_server_request(&mut stdin, &envelope)? {
            notifications.push(envelope);
            continue;
        }
        if let Some(error) = &envelope.error {
            if matches!(envelope.id, Some(0 | 1)) {
                bail!(
                    "Kiro ACP bootstrap failed for request {:?}: {}",
                    envelope.id,
                    error.message
                );
            }
            notifications.push(envelope);
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
    }

    let initialize = initialize.context("Kiro ACP bootstrap did not return initialize result")?;
    let session = session.context("Kiro ACP bootstrap did not return session/new result")?;
    drop(stdin);
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
    timeout: Duration,
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
    let lines = runtime_kiro_acp_line_receiver(stdout);
    let deadline = Instant::now() + timeout;
    let mut initialize = None;
    let mut session = None;
    let mut prompt_response = None;
    let mut notifications = Vec::new();
    let mut prompt_sent = false;
    loop {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            bail!("Kiro ACP prompt turn timed out");
        };
        let line = match lines.recv_timeout(remaining) {
            Ok(Ok(line)) => line,
            Ok(Err(error)) => return Err(error).context("failed to read Kiro ACP stdout"),
            Err(mpsc::RecvTimeoutError::Timeout) => bail!("Kiro ACP prompt turn timed out"),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        let current = line.trim();
        if current.is_empty() {
            continue;
        }
        let envelope = RuntimeKiroAcpEnvelope::parse(current)?;
        if runtime_kiro_acp_reject_unsupported_server_request(&mut stdin, &envelope)? {
            notifications.push(envelope);
            continue;
        }
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
    }
    drop(stdin);

    let initialize = initialize.context("Kiro ACP prompt turn did not return initialize result")?;
    let session = session.context("Kiro ACP prompt turn did not return session/new result")?;
    let prompt_response =
        prompt_response.context("Kiro ACP prompt turn did not return session/prompt response")?;
    Ok(RuntimeKiroAcpPromptTurnResult {
        initialize,
        session,
        prompt_response,
        notifications,
    })
}

pub(crate) fn runtime_kiro_acp_reject_unsupported_server_request(
    stdin: &mut impl Write,
    envelope: &RuntimeKiroAcpEnvelope,
) -> Result<bool> {
    let (Some(id), Some(_method)) = (envelope.id, envelope.method.as_deref()) else {
        return Ok(false);
    };
    writeln!(
        stdin,
        "{}",
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32601,
                "message": "Unsupported ACP server request",
            }
        })
    )
    .context("failed to reject unsupported Kiro ACP server request")?;
    stdin
        .flush()
        .context("failed to flush unsupported Kiro ACP server request response")?;
    Ok(true)
}

pub(crate) fn runtime_kiro_acp_line_receiver(
    stdout: std::process::ChildStdout,
) -> mpsc::Receiver<std::io::Result<String>> {
    let (sender, receiver) = mpsc::sync_channel(16);
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut received_bytes = 0;
        loop {
            match runtime_kiro_acp_read_line(&mut reader, &mut received_bytes) {
                Ok(None) => break,
                Ok(Some(line)) => {
                    if sender.send(Ok(line)).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = sender.send(Err(error));
                    break;
                }
            }
        }
    });
    receiver
}

fn runtime_kiro_acp_read_line(
    reader: &mut impl BufRead,
    received_bytes: &mut usize,
) -> io::Result<Option<String>> {
    let remaining = RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES.saturating_sub(*received_bytes);
    let mut line = String::new();
    let read = reader
        .by_ref()
        .take(remaining.saturating_add(1) as u64)
        .read_line(&mut line)?;
    if read == 0 {
        return Ok(None);
    }
    if read > remaining {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Kiro ACP output exceeded safe size limit ({RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES})"
            ),
        ));
    }
    *received_bytes += read;
    Ok(Some(line))
}

#[cfg(test)]
mod line_tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn kiro_acp_output_is_bounded_across_lines() {
        let first_line_bytes = RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES / 2;
        let mut input = vec![b'x'; first_line_bytes - 1];
        input.push(b'\n');
        input.extend(vec![
            b'y';
            RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES
                - first_line_bytes
                + 1
        ]);
        let mut reader = Cursor::new(input);
        let mut received_bytes = 0;

        assert!(
            runtime_kiro_acp_read_line(&mut reader, &mut received_bytes)
                .unwrap()
                .is_some()
        );

        let error = runtime_kiro_acp_read_line(&mut reader, &mut received_bytes).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("exceeded safe size limit"));
    }
}
