use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const RUNTIME_KIRO_ACP_STDERR_MAX_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpClientInfo<'a> {
    pub(crate) name: &'a str,
    pub(crate) title: &'a str,
    pub(crate) version: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpInitializeParams<'a> {
    pub(crate) protocol_version: u64,
    pub(crate) client_capabilities: RuntimeKiroAcpClientCapabilities,
    pub(crate) client_info: RuntimeKiroAcpClientInfo<'a>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpClientCapabilities {
    pub(crate) fs: RuntimeKiroAcpFsCapabilities,
    pub(crate) terminal: bool,
    pub(crate) auth: RuntimeKiroAcpAuthCapabilities,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpFsCapabilities {
    pub(crate) read_text_file: bool,
    pub(crate) write_text_file: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpAuthCapabilities {
    pub(crate) terminal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpSessionNewParams<'a> {
    pub(crate) cwd: &'a str,
    pub(crate) mcp_servers: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum RuntimeKiroAcpPromptPart<'a> {
    Text { text: &'a str },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpSessionPromptParams<'a> {
    pub(crate) session_id: &'a str,
    pub(crate) prompt: Vec<RuntimeKiroAcpPromptPart<'a>>,
}

pub(crate) fn runtime_kiro_acp_initialize_request(
    id: u64,
    client_info: RuntimeKiroAcpClientInfo<'_>,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": RuntimeKiroAcpInitializeParams {
            protocol_version: 1,
            client_capabilities: RuntimeKiroAcpClientCapabilities::default(),
            client_info,
        }
    })
}

pub(crate) fn runtime_kiro_acp_session_new_request(id: u64, cwd: &Path) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "session/new",
        "params": RuntimeKiroAcpSessionNewParams {
            cwd: &cwd.to_string_lossy(),
            mcp_servers: Vec::<Value>::new(),
        }
    })
}

pub(crate) fn runtime_kiro_acp_session_prompt_request(
    id: u64,
    session_id: &str,
    prompt: &str,
) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "session/prompt",
        "params": RuntimeKiroAcpSessionPromptParams {
            session_id,
            prompt: vec![RuntimeKiroAcpPromptPart::Text { text: prompt }],
        }
    })
}

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
    let _ = stderr
        .take(RUNTIME_KIRO_ACP_STDERR_MAX_BYTES)
        .read_to_string(&mut stderr_text);
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

pub(crate) fn runtime_kiro_acp_model_catalog(
    session: &RuntimeKiroAcpNewSessionResult,
) -> Vec<serde_json::Value> {
    session
        .models
        .as_ref()
        .map(|models| {
            models
                .available_models
                .iter()
                .map(|model| {
                    serde_json::json!({
                        "id": model.model_id,
                        "name": model.name,
                        "object": "model",
                        "owned_by": "kiro-cli",
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn runtime_kiro_acp_responses_value_from_prompt_turn(
    turn: &RuntimeKiroAcpPromptTurnResult,
    request_id: u64,
) -> serde_json::Value {
    let response_id = format!("resp_kiro_{request_id}");
    let model = turn
        .session
        .models
        .as_ref()
        .map(|models| models.current_model_id.clone())
        .unwrap_or_else(|| "kiro-cli".to_string());
    let turn_state = runtime_kiro_acp_collect_turn_state(turn);

    let mut output = Vec::new();
    if !turn_state.assistant_text.is_empty() {
        output.push(json!({
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": turn_state.assistant_text,
            }],
        }));
    }
    output.extend(
        turn_state
            .tool_calls
            .iter()
            .map(runtime_kiro_acp_responses_tool_call_item),
    );

    let mut response = json!({
        "id": response_id,
        "object": "response",
        "created_at": runtime_kiro_acp_created_at(),
        "model": model,
        "output": output,
    });

    if let Some(error) = &turn.prompt_response.error {
        response["status"] = Value::String("failed".to_string());
        response["error"] = json!({
            "code": error.code.to_string(),
            "message": error.message,
        });
    } else if let Some((reason, message)) =
        runtime_kiro_acp_incomplete_details(&turn.prompt_response)
    {
        response["status"] = Value::String("incomplete".to_string());
        response["incomplete_details"] = json!({
            "reason": reason,
            "message": message,
        });
    }

    let mut kiro_metadata = serde_json::Map::new();
    if !turn_state.reasoning_text.is_empty() {
        kiro_metadata.insert(
            "reasoning_content".to_string(),
            Value::String(turn_state.reasoning_text),
        );
    }
    if let Some(usage_update) = turn_state.usage_update {
        kiro_metadata.insert("usage_update".to_string(), usage_update);
    }
    if let Some(plan_entries) = turn_state.plan_entries {
        kiro_metadata.insert("plan".to_string(), Value::Array(plan_entries));
    }
    if let Some(available_commands) = turn_state.available_commands {
        kiro_metadata.insert(
            "available_commands".to_string(),
            Value::Array(available_commands),
        );
    }
    if let Some(current_mode_id) = turn_state.current_mode_id {
        kiro_metadata.insert(
            "current_mode_id".to_string(),
            Value::String(current_mode_id),
        );
    }
    if turn_state.session_title.is_some() || turn_state.session_updated_at.is_some() {
        kiro_metadata.insert(
            "session_info".to_string(),
            json!({
                "title": turn_state.session_title,
                "updated_at": turn_state.session_updated_at,
            }),
        );
    }
    if let Some(stop_reason) = runtime_kiro_acp_prompt_stop_reason(&turn.prompt_response) {
        kiro_metadata.insert("stop_reason".to_string(), Value::String(stop_reason));
    }
    if !kiro_metadata.is_empty() {
        response["metadata"] = json!({
            "kiro": kiro_metadata,
        });
    }
    response
}

pub(crate) fn runtime_kiro_acp_chat_assistant_messages_from_prompt_turn(
    turn: &RuntimeKiroAcpPromptTurnResult,
) -> Vec<serde_json::Value> {
    let turn_state = runtime_kiro_acp_collect_turn_state(turn);
    if turn_state.assistant_text.is_empty()
        && turn_state.reasoning_text.is_empty()
        && turn_state.tool_calls.is_empty()
    {
        return Vec::new();
    }
    let has_tool_calls = !turn_state.tool_calls.is_empty();
    let mut assistant = json!({
        "role": "assistant",
        "content": if turn_state.assistant_text.is_empty() {
            if has_tool_calls {
                Value::String(String::new())
            } else {
                Value::Null
            }
        } else {
            Value::String(turn_state.assistant_text)
        },
    });
    if !turn_state.reasoning_text.is_empty() {
        assistant["reasoning_content"] = Value::String(turn_state.reasoning_text);
    }
    if has_tool_calls {
        assistant["tool_calls"] = Value::Array(
            turn_state
                .tool_calls
                .iter()
                .map(runtime_kiro_acp_chat_tool_call_item)
                .collect(),
        );
    }
    vec![assistant]
}

fn runtime_kiro_acp_collect_turn_state(
    turn: &RuntimeKiroAcpPromptTurnResult,
) -> RuntimeKiroAcpTurnState {
    let mut assistant_text = String::new();
    let mut reasoning_text = String::new();
    let mut usage_update = None;
    let mut plan_entries = None;
    let mut available_commands = None;
    let mut current_mode_id = None;
    let mut session_title = None;
    let mut session_updated_at = None;
    let mut tool_calls = Vec::<RuntimeKiroAcpToolCallState>::new();
    let mut tool_call_indexes = BTreeMap::<String, usize>::new();

    for envelope in &turn.notifications {
        let Some(method) = envelope.method.as_deref() else {
            continue;
        };
        if method != "session/update" {
            continue;
        }
        let Ok(notification) = envelope.parse_session_notification() else {
            continue;
        };
        match notification.update {
            RuntimeKiroAcpSessionUpdate::AgentMessageChunk { content, .. } => {
                if let Some(text) = runtime_kiro_acp_content_text(&content) {
                    assistant_text.push_str(&text);
                }
            }
            RuntimeKiroAcpSessionUpdate::AgentThoughtChunk { content, .. } => {
                if let Some(text) = runtime_kiro_acp_content_text(&content) {
                    reasoning_text.push_str(&text);
                }
            }
            RuntimeKiroAcpSessionUpdate::ToolCall {
                tool_call_id,
                title,
                status,
                content,
                kind,
                raw_input,
                raw_output,
                locations,
            } => {
                let next_index = tool_calls.len();
                let index = *tool_call_indexes
                    .entry(tool_call_id.clone())
                    .or_insert(next_index);
                if index == next_index {
                    tool_calls.push(RuntimeKiroAcpToolCallState {
                        tool_call_id,
                        title: Some(title),
                        status: Some(status),
                        kind,
                        raw_input,
                        raw_output,
                        content,
                        locations,
                    });
                } else if let Some(existing) = tool_calls.get_mut(index) {
                    existing.title = Some(title);
                    existing.status = Some(status);
                    existing.kind = kind;
                    existing.raw_input = raw_input;
                    existing.raw_output = raw_output;
                    existing.content = content;
                    existing.locations = locations;
                }
            }
            RuntimeKiroAcpSessionUpdate::ToolCallUpdate {
                tool_call_id,
                title,
                status,
                content,
                kind,
                raw_input,
                raw_output,
                locations,
            } => {
                let next_index = tool_calls.len();
                let index = *tool_call_indexes
                    .entry(tool_call_id.clone())
                    .or_insert(next_index);
                if index == next_index {
                    tool_calls.push(RuntimeKiroAcpToolCallState {
                        tool_call_id,
                        title,
                        status,
                        kind,
                        raw_input,
                        raw_output,
                        content,
                        locations,
                    });
                } else if let Some(existing) = tool_calls.get_mut(index) {
                    if title.is_some() {
                        existing.title = title;
                    }
                    if status.is_some() {
                        existing.status = status;
                    }
                    if kind.is_some() {
                        existing.kind = kind;
                    }
                    if raw_input.is_some() {
                        existing.raw_input = raw_input;
                    }
                    if raw_output.is_some() {
                        existing.raw_output = raw_output;
                    }
                    if content.is_some() {
                        existing.content = content;
                    }
                    if locations.is_some() {
                        existing.locations = locations;
                    }
                }
            }
            RuntimeKiroAcpSessionUpdate::UsageUpdate { used, size, cost } => {
                usage_update = Some(runtime_kiro_acp_usage_update_json(used, size, cost));
            }
            RuntimeKiroAcpSessionUpdate::Plan { entries } => {
                plan_entries = Some(
                    entries
                        .into_iter()
                        .map(|entry| {
                            json!({
                                "content": entry.content,
                                "priority": entry.priority,
                                "status": entry.status,
                            })
                        })
                        .collect::<Vec<_>>(),
                );
            }
            RuntimeKiroAcpSessionUpdate::AvailableCommandsUpdate {
                available_commands: commands,
            } => {
                available_commands = Some(commands);
            }
            RuntimeKiroAcpSessionUpdate::CurrentModeUpdate {
                current_mode_id: mode_id,
            } => {
                current_mode_id = Some(mode_id);
            }
            RuntimeKiroAcpSessionUpdate::SessionInfoUpdate { title, updated_at } => {
                if title.is_some() {
                    session_title = title;
                }
                if updated_at.is_some() {
                    session_updated_at = updated_at;
                }
            }
            RuntimeKiroAcpSessionUpdate::UserMessageChunk { .. }
            | RuntimeKiroAcpSessionUpdate::Unknown { .. } => {}
        }
    }

    RuntimeKiroAcpTurnState {
        assistant_text,
        reasoning_text,
        usage_update,
        plan_entries,
        available_commands,
        current_mode_id,
        session_title,
        session_updated_at,
        tool_calls,
    }
}

fn runtime_kiro_acp_created_at() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn runtime_kiro_acp_content_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => (!text.is_empty()).then(|| text.clone()),
        Value::Array(items) => {
            let mut text = String::new();
            for item in items {
                if let Some(chunk) = runtime_kiro_acp_content_text(item) {
                    text.push_str(&chunk);
                }
            }
            (!text.is_empty()).then_some(text)
        }
        Value::Object(object) => {
            if let Some(text) = object.get("text").and_then(Value::as_str) {
                return (!text.is_empty()).then(|| text.to_string());
            }
            object
                .get("content")
                .and_then(runtime_kiro_acp_content_text)
        }
        _ => None,
    }
}

fn runtime_kiro_acp_prompt_stop_reason(envelope: &RuntimeKiroAcpEnvelope) -> Option<String> {
    envelope
        .result
        .as_ref()
        .and_then(|result| {
            result
                .get("stopReason")
                .or_else(|| result.get("stop_reason"))
                .or_else(|| result.get("status"))
        })
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn runtime_kiro_acp_incomplete_details(
    envelope: &RuntimeKiroAcpEnvelope,
) -> Option<(&'static str, &'static str)> {
    match runtime_kiro_acp_prompt_stop_reason(envelope).as_deref() {
        Some("max_tokens") => Some((
            "max_output_tokens",
            "Kiro stopped before end_turn because the model hit its output limit.",
        )),
        Some("max_turn_requests") => Some((
            "max_turn_requests",
            "Kiro stopped before end_turn because the turn hit its request limit.",
        )),
        Some("refusal") => Some(("refusal", "Kiro refused to continue the turn.")),
        Some("cancelled") => Some(("cancelled", "Kiro cancelled the turn before completion.")),
        _ => None,
    }
}

fn runtime_kiro_acp_usage_update_json(
    used: u64,
    size: u64,
    cost: Option<RuntimeKiroAcpCost>,
) -> Value {
    json!({
        "used": used,
        "size": size,
        "remaining": size.saturating_sub(used),
        "cost": cost.map(|cost| json!({
            "amount": cost.amount,
            "currency": cost.currency,
        })),
    })
}

fn runtime_kiro_acp_responses_tool_call_item(
    tool_call: &RuntimeKiroAcpToolCallState,
) -> serde_json::Value {
    let title = tool_call.title.as_deref().unwrap_or("tool_call");
    let name = runtime_kiro_acp_tool_name(tool_call.title.as_deref(), tool_call.kind.as_deref());
    let arguments = serde_json::to_string(tool_call.raw_input.as_ref().unwrap_or(&json!({})))
        .unwrap_or_else(|_| "{}".to_string());
    let mut item = json!({
        "type": "function_call",
        "call_id": tool_call.tool_call_id,
        "name": name,
        "namespace": "kiro",
        "arguments": arguments,
    });
    item["metadata"] = json!({
        "kiro": {
            "title": title,
            "status": tool_call.status,
            "kind": tool_call.kind,
            "raw_output": tool_call.raw_output,
            "content": tool_call.content,
            "locations": tool_call.locations,
        }
    });
    item
}

fn runtime_kiro_acp_chat_tool_call_item(
    tool_call: &RuntimeKiroAcpToolCallState,
) -> serde_json::Value {
    json!({
        "id": tool_call.tool_call_id,
        "type": "function",
        "function": {
            "name": runtime_kiro_acp_tool_name(tool_call.title.as_deref(), tool_call.kind.as_deref()),
            "arguments": serde_json::to_string(
                tool_call
                    .raw_input
                    .as_ref()
                    .unwrap_or(&json!({})),
            )
            .unwrap_or_else(|_| "{}".to_string()),
        },
    })
}

fn runtime_kiro_acp_tool_name(title: Option<&str>, kind: Option<&str>) -> String {
    let candidate = title
        .filter(|title| !title.trim().is_empty())
        .or(kind)
        .unwrap_or("tool_call");
    let mut normalized = String::new();
    let mut last_was_separator = false;
    for ch in candidate.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            last_was_separator = false;
        } else if !last_was_separator && !normalized.is_empty() {
            normalized.push('_');
            last_was_separator = true;
        }
    }
    normalized.trim_matches('_').to_string()
}

#[derive(Debug, Clone, PartialEq)]
struct RuntimeKiroAcpToolCallState {
    tool_call_id: String,
    title: Option<String>,
    status: Option<String>,
    kind: Option<String>,
    raw_input: Option<Value>,
    raw_output: Option<Value>,
    content: Option<Vec<Value>>,
    locations: Option<Vec<Value>>,
}

#[derive(Debug, Clone, PartialEq)]
struct RuntimeKiroAcpTurnState {
    assistant_text: String,
    reasoning_text: String,
    usage_update: Option<Value>,
    plan_entries: Option<Vec<Value>>,
    available_commands: Option<Vec<Value>>,
    current_mode_id: Option<String>,
    session_title: Option<String>,
    session_updated_at: Option<String>,
    tool_calls: Vec<RuntimeKiroAcpToolCallState>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct RuntimeKiroAcpEnvelope {
    pub(crate) jsonrpc: String,
    #[serde(default)]
    pub(crate) id: Option<u64>,
    #[serde(default)]
    pub(crate) method: Option<String>,
    #[serde(default)]
    pub(crate) params: Option<Value>,
    #[serde(default)]
    pub(crate) result: Option<Value>,
    #[serde(default)]
    pub(crate) error: Option<RuntimeKiroAcpError>,
}

impl RuntimeKiroAcpEnvelope {
    pub(crate) fn parse(line: &str) -> Result<Self> {
        let envelope: Self =
            serde_json::from_str(line).context("failed to parse Kiro ACP JSON-RPC line")?;
        if envelope.jsonrpc != "2.0" {
            bail!(
                "unsupported Kiro ACP jsonrpc version '{}'",
                envelope.jsonrpc
            );
        }
        Ok(envelope)
    }

    pub(crate) fn parse_initialize_result(&self) -> Result<RuntimeKiroAcpInitializeResult> {
        serde_json::from_value(
            self.result
                .clone()
                .context("missing Kiro ACP initialize result")?,
        )
        .context("failed to parse Kiro ACP initialize result")
    }

    pub(crate) fn parse_session_new_result(&self) -> Result<RuntimeKiroAcpNewSessionResult> {
        serde_json::from_value(
            self.result
                .clone()
                .context("missing Kiro ACP session/new result")?,
        )
        .context("failed to parse Kiro ACP session/new result")
    }

    pub(crate) fn parse_prompt_response(&self) -> Result<RuntimeKiroAcpPromptResponse> {
        serde_json::from_value(
            self.result
                .clone()
                .context("missing Kiro ACP session/prompt response")?,
        )
        .context("failed to parse Kiro ACP session/prompt response")
    }

    pub(crate) fn parse_session_notification(&self) -> Result<RuntimeKiroAcpSessionNotification> {
        if self.method.as_deref() != Some("session/update") {
            bail!(
                "expected Kiro ACP session/update notification, got {:?}",
                self.method
            );
        }
        let params = self
            .params
            .clone()
            .context("missing Kiro ACP session/update params")?;
        RuntimeKiroAcpSessionNotification::parse_value(params)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct RuntimeKiroAcpError {
    pub(crate) code: i64,
    pub(crate) message: String,
    #[serde(default)]
    pub(crate) data: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpInitializeResult {
    pub(crate) protocol_version: u64,
    pub(crate) agent_capabilities: RuntimeKiroAcpAgentCapabilities,
    #[serde(default)]
    pub(crate) auth_methods: Vec<RuntimeKiroAcpAuthMethod>,
    pub(crate) agent_info: RuntimeKiroAcpAgentInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpAgentCapabilities {
    pub(crate) load_session: bool,
    pub(crate) prompt_capabilities: RuntimeKiroAcpPromptCapabilities,
    pub(crate) mcp_capabilities: RuntimeKiroAcpMcpCapabilities,
    #[serde(default)]
    pub(crate) session_capabilities: Value,
    #[serde(default)]
    pub(crate) auth: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpPromptCapabilities {
    pub(crate) image: bool,
    pub(crate) audio: bool,
    pub(crate) embedded_context: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpMcpCapabilities {
    pub(crate) http: bool,
    pub(crate) sse: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct RuntimeKiroAcpAuthMethod {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct RuntimeKiroAcpAgentInfo {
    pub(crate) name: String,
    pub(crate) title: String,
    pub(crate) version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpNewSessionResult {
    pub(crate) session_id: String,
    #[serde(default)]
    pub(crate) modes: Option<RuntimeKiroAcpModeState>,
    #[serde(default)]
    pub(crate) models: Option<RuntimeKiroAcpModelState>,
}

impl RuntimeKiroAcpNewSessionResult {
    pub(crate) fn model_ids(&self) -> Vec<&str> {
        self.models
            .as_ref()
            .map(|models| {
                models
                    .available_models
                    .iter()
                    .map(|model| model.model_id.as_str())
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpPromptResponse {
    #[serde(alias = "stop_reason")]
    pub(crate) stop_reason: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpSessionNotification {
    pub(crate) session_id: String,
    pub(crate) update: RuntimeKiroAcpSessionUpdate,
}

impl RuntimeKiroAcpSessionNotification {
    fn parse_value(value: Value) -> Result<Self> {
        let session_id = value
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_string)
            .context("Kiro ACP session/update is missing sessionId")?;
        let update_value = value
            .get("update")
            .cloned()
            .context("Kiro ACP session/update is missing update payload")?;
        let update = RuntimeKiroAcpSessionUpdate::parse_value(update_value)?;
        Ok(Self { session_id, update })
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) enum RuntimeKiroAcpSessionUpdate {
    UserMessageChunk {
        message_id: Option<String>,
        content: Value,
    },
    AgentMessageChunk {
        message_id: Option<String>,
        content: Value,
    },
    AgentThoughtChunk {
        message_id: Option<String>,
        content: Value,
    },
    ToolCall {
        tool_call_id: String,
        title: String,
        status: String,
        content: Option<Vec<Value>>,
        kind: Option<String>,
        raw_input: Option<Value>,
        raw_output: Option<Value>,
        locations: Option<Vec<Value>>,
    },
    ToolCallUpdate {
        tool_call_id: String,
        title: Option<String>,
        status: Option<String>,
        content: Option<Vec<Value>>,
        kind: Option<String>,
        raw_input: Option<Value>,
        raw_output: Option<Value>,
        locations: Option<Vec<Value>>,
    },
    Plan {
        entries: Vec<RuntimeKiroAcpPlanEntry>,
    },
    UsageUpdate {
        used: u64,
        size: u64,
        cost: Option<RuntimeKiroAcpCost>,
    },
    SessionInfoUpdate {
        title: Option<String>,
        updated_at: Option<String>,
    },
    AvailableCommandsUpdate {
        available_commands: Vec<Value>,
    },
    CurrentModeUpdate {
        current_mode_id: String,
    },
    Unknown {
        session_update: String,
        raw: Value,
    },
}

impl RuntimeKiroAcpSessionUpdate {
    fn parse_value(value: Value) -> Result<Self> {
        let session_update = value
            .get("sessionUpdate")
            .and_then(Value::as_str)
            .map(str::to_string)
            .context("Kiro ACP session/update is missing sessionUpdate discriminator")?;
        Ok(match session_update.as_str() {
            "user_message_chunk" => Self::UserMessageChunk {
                message_id: value
                    .get("messageId")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                content: value
                    .get("content")
                    .cloned()
                    .context("user_message_chunk is missing content")?,
            },
            "agent_message_chunk" => Self::AgentMessageChunk {
                message_id: value
                    .get("messageId")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                content: value
                    .get("content")
                    .cloned()
                    .context("agent_message_chunk is missing content")?,
            },
            "agent_thought_chunk" => Self::AgentThoughtChunk {
                message_id: value
                    .get("messageId")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                content: value
                    .get("content")
                    .cloned()
                    .context("agent_thought_chunk is missing content")?,
            },
            "tool_call" => Self::ToolCall {
                tool_call_id: value
                    .get("toolCallId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .context("tool_call is missing toolCallId")?,
                title: value
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .context("tool_call is missing title")?,
                status: value
                    .get("status")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .context("tool_call is missing status")?,
                content: value.get("content").and_then(Value::as_array).cloned(),
                kind: value
                    .get("kind")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                raw_input: value.get("rawInput").cloned(),
                raw_output: value.get("rawOutput").cloned(),
                locations: value.get("locations").and_then(Value::as_array).cloned(),
            },
            "tool_call_update" => Self::ToolCallUpdate {
                tool_call_id: value
                    .get("toolCallId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .context("tool_call_update is missing toolCallId")?,
                title: value
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                status: value
                    .get("status")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                content: value.get("content").and_then(Value::as_array).cloned(),
                kind: value
                    .get("kind")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                raw_input: value.get("rawInput").cloned(),
                raw_output: value.get("rawOutput").cloned(),
                locations: value.get("locations").and_then(Value::as_array).cloned(),
            },
            "plan" => Self::Plan {
                entries: serde_json::from_value(
                    value
                        .get("entries")
                        .cloned()
                        .context("plan update is missing entries")?,
                )
                .context("failed to parse plan entries")?,
            },
            "usage_update" => Self::UsageUpdate {
                used: value
                    .get("used")
                    .and_then(Value::as_u64)
                    .context("usage_update is missing used")?,
                size: value
                    .get("size")
                    .and_then(Value::as_u64)
                    .context("usage_update is missing size")?,
                cost: value
                    .get("cost")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()
                    .context("failed to parse usage_update cost")?,
            },
            "session_info_update" => Self::SessionInfoUpdate {
                title: value
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                updated_at: value
                    .get("updatedAt")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            },
            "available_commands_update" => Self::AvailableCommandsUpdate {
                available_commands: value
                    .get("availableCommands")
                    .and_then(Value::as_array)
                    .cloned()
                    .context("available_commands_update is missing availableCommands")?,
            },
            "current_mode_update" => Self::CurrentModeUpdate {
                current_mode_id: value
                    .get("currentModeId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .context("current_mode_update is missing currentModeId")?,
            },
            _ => Self::Unknown {
                session_update,
                raw: value,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct RuntimeKiroAcpPlanEntry {
    pub(crate) content: String,
    pub(crate) priority: String,
    pub(crate) status: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct RuntimeKiroAcpCost {
    pub(crate) amount: f64,
    pub(crate) currency: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpModeState {
    pub(crate) current_mode_id: String,
    #[serde(default)]
    pub(crate) available_modes: Vec<RuntimeKiroAcpMode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct RuntimeKiroAcpMode {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
    #[serde(default, rename = "_meta")]
    pub(crate) meta: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpModelState {
    pub(crate) current_model_id: String,
    #[serde(default)]
    pub(crate) available_models: Vec<RuntimeKiroAcpModelInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeKiroAcpModelInfo {
    pub(crate) model_id: String,
    pub(crate) name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "prodex-kiro-acp-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("temp dir should exist");
        dir
    }

    fn write_fake_kiro_acp_agent(root: &Path) -> std::path::PathBuf {
        let script = root.join("fake-kiro");
        fs::write(
            &script,
            r#"#!/usr/bin/env python3
import json, sys
first = json.loads(sys.stdin.readline())
second = json.loads(sys.stdin.readline())
assert first["method"] == "initialize"
assert second["method"] == "session/new"
print(json.dumps({"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":True,"promptCapabilities":{"image":True,"audio":False,"embeddedContext":False},"mcpCapabilities":{"http":True,"sse":False},"sessionCapabilities":{},"auth":{}},"authMethods":[{"id":"kiro-login","name":"Kiro Login","description":"Run 'kiro-cli login'."}],"agentInfo":{"name":"Kiro CLI Agent","title":"Kiro CLI Agent","version":"2.10.0"}},"id":0}), flush=True)
print(json.dumps({"jsonrpc":"2.0","method":"_kiro.dev/subagent/list_update","params":{"subagents":[],"pendingStages":[]}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"sessionId":"session-1","modes":{"currentModeId":"kiro_default","availableModes":[{"id":"kiro_default","name":"kiro_default","description":"The default agent for Kiro CLI"}]},"models":{"currentModelId":"claude-sonnet-4","availableModels":[{"modelId":"claude-sonnet-4","name":"claude-sonnet-4"},{"modelId":"claude-sonnet-4.5","name":"claude-sonnet-4.5"}]}},"id":1}), flush=True)
"#,
        )
        .expect("fake agent should be written");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script).expect("metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script, perms).expect("permissions should update");
        }
        script
    }

    fn write_fake_kiro_prompt_agent(root: &Path) -> std::path::PathBuf {
        let script = root.join("fake-kiro-prompt");
        fs::write(
            &script,
            r#"#!/usr/bin/env python3
import json, sys
first = json.loads(sys.stdin.readline())
second = json.loads(sys.stdin.readline())
assert first["method"] == "initialize"
assert second["method"] == "session/new"
print(json.dumps({"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":True,"promptCapabilities":{"image":True,"audio":False,"embeddedContext":False},"mcpCapabilities":{"http":True,"sse":False},"sessionCapabilities":{},"auth":{}},"authMethods":[{"id":"kiro-login","name":"Kiro Login","description":"Run 'kiro-cli login'."}],"agentInfo":{"name":"Kiro CLI Agent","title":"Kiro CLI Agent","version":"2.10.0"}},"id":0}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"sessionId":"session-1","models":{"currentModelId":"claude-sonnet-4","availableModels":[{"modelId":"claude-sonnet-4","name":"claude-sonnet-4"}]}},"id":1}), flush=True)
third = json.loads(sys.stdin.readline())
assert third["method"] == "session/prompt"
assert third["params"]["sessionId"] == "session-1"
assert third["params"]["prompt"][0]["text"] == "hello from prodex"
print(json.dumps({"jsonrpc":"2.0","method":"_kiro.dev/metadata","params":{"sessionId":"session-1","turnDurationMs":8}}), flush=True)
print(json.dumps({"jsonrpc":"2.0","result":{"status":"completed"},"id":2}), flush=True)
"#,
        )
        .expect("fake prompt agent should be written");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script).expect("metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script, perms).expect("permissions should update");
        }
        script
    }

    #[test]
    fn kiro_acp_builds_initialize_and_session_requests() {
        let initialize = runtime_kiro_acp_initialize_request(
            0,
            RuntimeKiroAcpClientInfo {
                name: "acp-test-client",
                title: "ACP Test Client",
                version: "0.1.0",
            },
        );
        assert_eq!(initialize["method"], "initialize");
        assert_eq!(initialize["params"]["protocolVersion"], 1);
        assert_eq!(
            initialize["params"]["clientCapabilities"]["terminal"],
            false
        );

        let session_new = runtime_kiro_acp_session_new_request(1, Path::new("/tmp/work"));
        assert_eq!(session_new["method"], "session/new");
        assert_eq!(session_new["params"]["cwd"], "/tmp/work");
        assert_eq!(session_new["params"]["mcpServers"], json!([]));

        let prompt = runtime_kiro_acp_session_prompt_request(2, "session-1", "hello");
        assert_eq!(prompt["method"], "session/prompt");
        assert_eq!(prompt["params"]["sessionId"], "session-1");
        assert_eq!(
            prompt["params"]["prompt"][0],
            json!({"type":"text","text":"hello"})
        );
    }

    #[test]
    fn kiro_acp_parses_initialize_result_from_captured_agent_line() {
        let envelope = RuntimeKiroAcpEnvelope::parse(
            r#"{"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":true,"promptCapabilities":{"image":true,"audio":false,"embeddedContext":false},"mcpCapabilities":{"http":true,"sse":false},"sessionCapabilities":{},"auth":{}},"authMethods":[{"id":"kiro-login","name":"Kiro Login","description":"Run 'kiro-cli login' in terminal to authenticate. See https://kiro.dev/docs/cli/authentication/"}],"agentInfo":{"name":"Kiro CLI Agent","title":"Kiro CLI Agent","version":"2.10.0"}},"id":0}"#,
        )
        .expect("initialize envelope should parse");
        let result = envelope
            .parse_initialize_result()
            .expect("initialize result should parse");
        assert_eq!(result.protocol_version, 1);
        assert!(result.agent_capabilities.load_session);
        assert_eq!(result.agent_info.version, "2.10.0");
        assert_eq!(result.auth_methods[0].id, "kiro-login");
    }

    #[test]
    fn kiro_acp_parses_new_session_result_and_model_ids() {
        let envelope = RuntimeKiroAcpEnvelope::parse(
            r#"{"jsonrpc":"2.0","result":{"sessionId":"43dc1cdb-4376-442a-afa8-b12238274893","modes":{"currentModeId":"kiro_default","availableModes":[{"id":"kiro_default","name":"kiro_default","description":"The default agent for Kiro CLI"}]},"models":{"currentModelId":"claude-sonnet-4","availableModels":[{"modelId":"claude-sonnet-4","name":"claude-sonnet-4"},{"modelId":"claude-sonnet-4.5","name":"claude-sonnet-4.5"}]}},"id":1}"#,
        )
        .expect("session/new envelope should parse");
        let result = envelope
            .parse_session_new_result()
            .expect("session/new result should parse");
        assert_eq!(result.session_id, "43dc1cdb-4376-442a-afa8-b12238274893");
        assert_eq!(
            result.model_ids(),
            vec!["claude-sonnet-4", "claude-sonnet-4.5"]
        );
        assert_eq!(
            result
                .modes
                .as_ref()
                .expect("modes should be present")
                .current_mode_id,
            "kiro_default"
        );
    }

    #[test]
    fn kiro_acp_parses_error_notification_line() {
        let envelope = RuntimeKiroAcpEnvelope::parse(
            r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"Internal error","data":"Encountered an error in the response stream: An unknown error occurred: dispatch failure"},"id":2}"#,
        )
        .expect("error envelope should parse");
        let error = envelope.error.expect("error should be present");
        assert_eq!(error.code, -32603);
        assert_eq!(error.message, "Internal error");
        assert_eq!(
            error.data,
            Some(Value::String(
                "Encountered an error in the response stream: An unknown error occurred: dispatch failure"
                    .to_string()
            ))
        );
    }

    #[test]
    fn kiro_acp_bootstrap_reads_initialize_session_and_notifications() {
        let root = temp_dir("bootstrap");
        let fake_agent = write_fake_kiro_acp_agent(&root);
        let result = runtime_kiro_acp_bootstrap_with_command(fake_agent.as_os_str(), &root, &[])
            .expect("bootstrap should succeed");
        assert_eq!(result.initialize.agent_info.version, "2.10.0");
        assert_eq!(result.session.session_id, "session-1");
        assert_eq!(
            result.session.model_ids(),
            vec!["claude-sonnet-4", "claude-sonnet-4.5"]
        );
        assert_eq!(result.notifications.len(), 1);
        assert_eq!(
            result.notifications[0].method.as_deref(),
            Some("_kiro.dev/subagent/list_update")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn kiro_acp_model_catalog_maps_session_models() {
        let session = RuntimeKiroAcpNewSessionResult {
            session_id: "session-1".to_string(),
            modes: None,
            models: Some(RuntimeKiroAcpModelState {
                current_model_id: "claude-sonnet-4".to_string(),
                available_models: vec![
                    RuntimeKiroAcpModelInfo {
                        model_id: "claude-sonnet-4".to_string(),
                        name: "claude-sonnet-4".to_string(),
                    },
                    RuntimeKiroAcpModelInfo {
                        model_id: "claude-sonnet-4.5".to_string(),
                        name: "claude-sonnet-4.5".to_string(),
                    },
                ],
            }),
        };
        let catalog = runtime_kiro_acp_model_catalog(&session);
        assert_eq!(catalog.len(), 2);
        assert_eq!(catalog[0]["id"], "claude-sonnet-4");
        assert_eq!(catalog[0]["owned_by"], "kiro-cli");
    }

    #[test]
    fn kiro_acp_prompt_turn_sends_prompt_after_session_bootstrap() {
        let root = temp_dir("prompt-turn");
        let fake_agent = write_fake_kiro_prompt_agent(&root);
        let result = runtime_kiro_acp_prompt_turn_with_command(
            fake_agent.as_os_str(),
            &root,
            &[],
            "hello from prodex",
        )
        .expect("prompt turn should succeed");
        assert_eq!(result.initialize.agent_info.version, "2.10.0");
        assert_eq!(result.session.session_id, "session-1");
        assert_eq!(result.prompt_response.id, Some(2));
        assert_eq!(
            result.prompt_response.result,
            Some(json!({"status":"completed"}))
        );
        assert_eq!(result.notifications.len(), 1);
        assert_eq!(
            result.notifications[0].method.as_deref(),
            Some("_kiro.dev/metadata")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn kiro_acp_parses_prompt_response_stop_reason() {
        let envelope = RuntimeKiroAcpEnvelope::parse(
            r#"{"jsonrpc":"2.0","id":2,"result":{"stopReason":"end_turn"}}"#,
        )
        .expect("prompt response envelope should parse");
        let response = envelope
            .parse_prompt_response()
            .expect("prompt response should parse");
        assert_eq!(response.stop_reason, "end_turn");
    }

    #[test]
    fn kiro_acp_parses_session_update_agent_message_chunk() {
        let envelope = RuntimeKiroAcpEnvelope::parse(
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"sess_abc123def456","update":{"sessionUpdate":"agent_message_chunk","messageId":"msg_agent_c42b9","content":{"type":"text","text":"I'll analyze your code for potential issues. Let me examine it..."}}}}"#,
        )
        .expect("session/update envelope should parse");
        let notification = envelope
            .parse_session_notification()
            .expect("session/update should parse");
        assert_eq!(notification.session_id, "sess_abc123def456");
        match notification.update {
            RuntimeKiroAcpSessionUpdate::AgentMessageChunk {
                message_id,
                content,
            } => {
                assert_eq!(message_id.as_deref(), Some("msg_agent_c42b9"));
                assert_eq!(content["type"], "text");
            }
            other => panic!("expected agent message chunk, got {other:?}"),
        }
    }

    #[test]
    fn kiro_acp_parses_session_update_usage_update() {
        let envelope = RuntimeKiroAcpEnvelope::parse(
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"sess_abc123def456","update":{"sessionUpdate":"usage_update","used":53000,"size":200000,"cost":{"amount":0.045,"currency":"USD"}}}}"#,
        )
        .expect("usage update envelope should parse");
        let notification = envelope
            .parse_session_notification()
            .expect("usage update should parse");
        match notification.update {
            RuntimeKiroAcpSessionUpdate::UsageUpdate { used, size, cost } => {
                assert_eq!(used, 53_000);
                assert_eq!(size, 200_000);
                assert_eq!(cost.expect("cost should exist").currency, "USD");
            }
            other => panic!("expected usage update, got {other:?}"),
        }
    }

    #[test]
    fn kiro_acp_parses_session_update_tool_call() {
        let envelope = RuntimeKiroAcpEnvelope::parse(
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"sess_abc123def456","update":{"sessionUpdate":"tool_call","toolCallId":"call_1","title":"Read file","status":"in_progress","kind":"read","content":[{"type":"resource_link","uri":"file:///tmp/main.py"}],"rawInput":{"path":"/tmp/main.py"},"rawOutput":null,"locations":[]}}}"#,
        )
        .expect("tool call envelope should parse");
        let notification = envelope
            .parse_session_notification()
            .expect("tool call should parse");
        match notification.update {
            RuntimeKiroAcpSessionUpdate::ToolCall {
                tool_call_id,
                title,
                status,
                kind,
                ..
            } => {
                assert_eq!(tool_call_id, "call_1");
                assert_eq!(title, "Read file");
                assert_eq!(status, "in_progress");
                assert_eq!(kind.as_deref(), Some("read"));
            }
            other => panic!("expected tool call, got {other:?}"),
        }
    }

    #[test]
    fn kiro_acp_prompt_turn_translates_text_response() {
        let turn = RuntimeKiroAcpPromptTurnResult {
            initialize: RuntimeKiroAcpInitializeResult {
                protocol_version: 1,
                agent_capabilities: RuntimeKiroAcpAgentCapabilities {
                    load_session: true,
                    prompt_capabilities: RuntimeKiroAcpPromptCapabilities {
                        image: false,
                        audio: false,
                        embedded_context: false,
                    },
                    mcp_capabilities: RuntimeKiroAcpMcpCapabilities {
                        http: true,
                        sse: false,
                    },
                    session_capabilities: json!({}),
                    auth: json!({}),
                },
                auth_methods: Vec::new(),
                agent_info: RuntimeKiroAcpAgentInfo {
                    name: "Kiro CLI Agent".to_string(),
                    title: "Kiro CLI Agent".to_string(),
                    version: "2.10.0".to_string(),
                },
            },
            session: RuntimeKiroAcpNewSessionResult {
                session_id: "session-1".to_string(),
                modes: None,
                models: Some(RuntimeKiroAcpModelState {
                    current_model_id: "claude-sonnet-4".to_string(),
                    available_models: vec![RuntimeKiroAcpModelInfo {
                        model_id: "claude-sonnet-4".to_string(),
                        name: "claude-sonnet-4".to_string(),
                    }],
                }),
            },
            prompt_response: RuntimeKiroAcpEnvelope::parse(
                r#"{"jsonrpc":"2.0","id":2,"result":{"stopReason":"end_turn"}}"#,
            )
            .expect("prompt response should parse"),
            notifications: vec![
                RuntimeKiroAcpEnvelope::parse(
                    r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"agent_message_chunk","messageId":"msg_1","content":{"type":"text","text":"hello "}}}}"#,
                )
                .expect("text chunk should parse"),
                RuntimeKiroAcpEnvelope::parse(
                    r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"agent_message_chunk","messageId":"msg_1","content":{"type":"text","text":"world"}}}}"#,
                )
                .expect("text chunk should parse"),
                RuntimeKiroAcpEnvelope::parse(
                    r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"agent_thought_chunk","messageId":"thought_1","content":{"type":"text","text":"inspect code"}}}}"#,
                )
                .expect("thought chunk should parse"),
            ],
        };

        let response = runtime_kiro_acp_responses_value_from_prompt_turn(&turn, 7);
        assert_eq!(response["id"], "resp_kiro_7");
        assert_eq!(response["object"], "response");
        assert_eq!(response["model"], "claude-sonnet-4");
        assert_eq!(response["output"][0]["type"], "message");
        assert_eq!(response["output"][0]["content"][0]["type"], "output_text");
        assert_eq!(response["output"][0]["content"][0]["text"], "hello world");
        assert_eq!(
            response["metadata"]["kiro"]["reasoning_content"],
            "inspect code"
        );
        assert_eq!(response["metadata"]["kiro"]["stop_reason"], "end_turn");
    }

    #[test]
    fn kiro_acp_prompt_turn_translates_tool_call_response() {
        let turn = RuntimeKiroAcpPromptTurnResult {
            initialize: RuntimeKiroAcpInitializeResult {
                protocol_version: 1,
                agent_capabilities: RuntimeKiroAcpAgentCapabilities {
                    load_session: true,
                    prompt_capabilities: RuntimeKiroAcpPromptCapabilities {
                        image: false,
                        audio: false,
                        embedded_context: false,
                    },
                    mcp_capabilities: RuntimeKiroAcpMcpCapabilities {
                        http: true,
                        sse: false,
                    },
                    session_capabilities: json!({}),
                    auth: json!({}),
                },
                auth_methods: Vec::new(),
                agent_info: RuntimeKiroAcpAgentInfo {
                    name: "Kiro CLI Agent".to_string(),
                    title: "Kiro CLI Agent".to_string(),
                    version: "2.10.0".to_string(),
                },
            },
            session: RuntimeKiroAcpNewSessionResult {
                session_id: "session-1".to_string(),
                modes: None,
                models: Some(RuntimeKiroAcpModelState {
                    current_model_id: "claude-sonnet-4".to_string(),
                    available_models: vec![RuntimeKiroAcpModelInfo {
                        model_id: "claude-sonnet-4".to_string(),
                        name: "claude-sonnet-4".to_string(),
                    }],
                }),
            },
            prompt_response: RuntimeKiroAcpEnvelope::parse(
                r#"{"jsonrpc":"2.0","id":2,"result":{"stopReason":"end_turn"}}"#,
            )
            .expect("prompt response should parse"),
            notifications: vec![
                RuntimeKiroAcpEnvelope::parse(
                    r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"tool_call","toolCallId":"call_1","title":"Read file","status":"in_progress","kind":"read","rawInput":{"path":"/tmp/main.py"}}}}"#,
                )
                .expect("tool call should parse"),
                RuntimeKiroAcpEnvelope::parse(
                    r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"tool_call_update","toolCallId":"call_1","status":"completed","rawOutput":{"content":"ok"}}}}"#,
                )
                .expect("tool call update should parse"),
            ],
        };

        let response = runtime_kiro_acp_responses_value_from_prompt_turn(&turn, 8);
        assert_eq!(response["output"][0]["type"], "function_call");
        assert_eq!(response["output"][0]["call_id"], "call_1");
        assert_eq!(response["output"][0]["name"], "read_file");
        assert_eq!(response["output"][0]["namespace"], "kiro");
        assert_eq!(
            response["output"][0]["arguments"],
            r#"{"path":"/tmp/main.py"}"#
        );
        assert_eq!(
            response["output"][0]["metadata"]["kiro"]["status"],
            "completed"
        );
        assert_eq!(
            response["output"][0]["metadata"]["kiro"]["raw_output"]["content"],
            "ok"
        );
    }

    #[test]
    fn kiro_acp_prompt_turn_carries_usage_and_incomplete_metadata() {
        let turn = RuntimeKiroAcpPromptTurnResult {
            initialize: RuntimeKiroAcpInitializeResult {
                protocol_version: 1,
                agent_capabilities: RuntimeKiroAcpAgentCapabilities {
                    load_session: true,
                    prompt_capabilities: RuntimeKiroAcpPromptCapabilities {
                        image: false,
                        audio: false,
                        embedded_context: false,
                    },
                    mcp_capabilities: RuntimeKiroAcpMcpCapabilities {
                        http: true,
                        sse: false,
                    },
                    session_capabilities: json!({}),
                    auth: json!({}),
                },
                auth_methods: Vec::new(),
                agent_info: RuntimeKiroAcpAgentInfo {
                    name: "Kiro CLI Agent".to_string(),
                    title: "Kiro CLI Agent".to_string(),
                    version: "2.10.0".to_string(),
                },
            },
            session: RuntimeKiroAcpNewSessionResult {
                session_id: "session-1".to_string(),
                modes: None,
                models: Some(RuntimeKiroAcpModelState {
                    current_model_id: "claude-sonnet-4".to_string(),
                    available_models: vec![RuntimeKiroAcpModelInfo {
                        model_id: "claude-sonnet-4".to_string(),
                        name: "claude-sonnet-4".to_string(),
                    }],
                }),
            },
            prompt_response: RuntimeKiroAcpEnvelope::parse(
                r#"{"jsonrpc":"2.0","id":2,"result":{"stopReason":"max_tokens"}}"#,
            )
            .expect("prompt response should parse"),
            notifications: vec![RuntimeKiroAcpEnvelope::parse(
                r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{"sessionUpdate":"usage_update","used":53000,"size":200000,"cost":{"amount":0.045,"currency":"USD"}}}}"#,
            )
            .expect("usage update should parse")],
        };

        let response = runtime_kiro_acp_responses_value_from_prompt_turn(&turn, 9);
        assert_eq!(response["status"], "incomplete");
        assert_eq!(
            response["incomplete_details"]["reason"],
            "max_output_tokens"
        );
        assert_eq!(response["metadata"]["kiro"]["usage_update"]["used"], 53_000);
        assert_eq!(
            response["metadata"]["kiro"]["usage_update"]["size"],
            200_000
        );
        assert_eq!(
            response["metadata"]["kiro"]["usage_update"]["remaining"],
            147_000
        );
        assert_eq!(
            response["metadata"]["kiro"]["usage_update"]["cost"]["currency"],
            "USD"
        );
    }
}
