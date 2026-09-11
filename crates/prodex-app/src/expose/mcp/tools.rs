use super::MCP_MAX_EVENT_PAGE;
use super::exec::{
    EXEC_DEFAULT_TIMEOUT_MS, EXEC_MAX_ARGUMENT_BYTES, EXEC_MAX_ARGUMENTS, EXEC_MAX_ARGUMENTS_BYTES,
    EXEC_MAX_CWD_BYTES, EXEC_MAX_ENV_ENTRIES, EXEC_MAX_ENV_KEY_BYTES, EXEC_MAX_ENV_VALUE_BYTES,
    EXEC_MAX_OUTPUT_BYTES, EXEC_MAX_PROGRAM_BYTES, EXEC_MAX_STDIN_BYTES, EXEC_MAX_TIMEOUT_MS,
};
use crate::expose::run_manager::{EXPOSE_MAX_RUN_ID_BYTES, ExposeRunSummary};
use prodex_cli::SuperArgs;
use serde_json::{Value, json};
use std::ffi::OsString;

pub(super) fn tool_result(value: Value, is_error: bool) -> Value {
    json!({
        "resultType": "complete",
        "content": [{"type": "text", "text": serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string())}],
        "structuredContent": value,
        "isError": is_error,
    })
}

pub(super) fn required_string(
    arguments: &Value,
    name: &str,
    max_bytes: usize,
) -> std::result::Result<String, String> {
    let Some(value) = arguments.get(name).and_then(Value::as_str) else {
        return Err(format!("{name} is required"));
    };
    if value.is_empty() || value.len() > max_bytes {
        return Err(format!("{name} is empty or too large"));
    }
    Ok(value.to_string())
}

pub(super) fn optional_string(
    arguments: &Value,
    name: &str,
    max_bytes: usize,
) -> std::result::Result<Option<String>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let Some(value) = value.as_str() else {
        return Err(format!("{name} must be a string"));
    };
    if value.is_empty()
        || value.len() > max_bytes
        || value.as_bytes().contains(&0)
        || value.chars().any(char::is_control)
    {
        return Err(format!("{name} is empty or too large"));
    }
    Ok(Some(value.to_string()))
}

pub(super) fn validate_tool_arguments(
    tool: &str,
    arguments: &Value,
) -> std::result::Result<(), String> {
    let allowed = match tool {
        "prodex_super_start" => [
            "task",
            "model",
            "reasoning_effort",
            "provider",
            "profile",
            "sub_agents",
        ]
        .as_slice(),
        "prodex_super_status" | "prodex_super_result" | "prodex_super_cancel" => {
            ["run_id"].as_slice()
        }
        "prodex_super_events" => ["run_id", "after_seq", "limit"].as_slice(),
        "prodex_super_list" => [].as_slice(),
        "prodex_session_prompt_write" => ["message", "cwd", "prodex_pid", "thread_id"].as_slice(),
        "prodex_session_preempt" => ["cwd", "prodex_pid", "thread_id"].as_slice(),
        "prodex_session_output_read" => {
            ["cursor", "limit", "wait_ms", "prodex_pid", "thread_id"].as_slice()
        }
        "prodex_super_exec" => ["program", "args", "cwd", "env", "stdin", "timeout_ms"].as_slice(),
        _ => return Ok(()),
    };
    let Some(object) = arguments.as_object() else {
        return Err("tool arguments must be an object".to_string());
    };
    if let Some(unknown) = object
        .keys()
        .find(|key| !allowed.iter().any(|candidate| candidate == key))
    {
        return Err(format!("unknown tool argument: {unknown}"));
    }
    Ok(())
}

pub(super) fn required_run_id(arguments: &Value) -> std::result::Result<String, String> {
    let run_id = required_string(arguments, "run_id", EXPOSE_MAX_RUN_ID_BYTES)?;
    if !run_id.starts_with("spr_")
        || !run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err("run_id is invalid".to_string());
    }
    Ok(run_id)
}

pub(super) fn value_u64(value: &Value) -> std::result::Result<u64, String> {
    value
        .as_u64()
        .ok_or_else(|| "value must be a nonnegative integer".to_string())
}

pub(super) fn value_usize(value: &Value) -> std::result::Result<usize, String> {
    value_u64(value)
        .and_then(|value| usize::try_from(value).map_err(|_| "value is too large".to_string()))
}

pub(super) fn optional_process_id(arguments: &Value) -> std::result::Result<Option<u32>, String> {
    let Some(value) = arguments.get("prodex_pid") else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let value = value_u64(value)?;
    u32::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .map(Some)
        .ok_or_else(|| "prodex_pid is invalid".to_string())
}

pub(super) fn event_page_limit(arguments: &Value) -> std::result::Result<usize, String> {
    match arguments.get("limit").map(value_usize).transpose()? {
        Some(limit) if (1..=MCP_MAX_EVENT_PAGE).contains(&limit) => Ok(limit),
        Some(_) => Err(format!("limit must be between 1 and {MCP_MAX_EVENT_PAGE}")),
        None => Ok(MCP_MAX_EVENT_PAGE),
    }
}

pub(super) fn main_provider(args: &SuperArgs) -> prodex_provider_core::ProviderId {
    args.url
        .as_ref()
        .map(|_| prodex_provider_core::ProviderId::Local)
        .or_else(|| {
            args.provider
                .map(prodex_cli::SuperExternalProvider::provider_id)
        })
        .or_else(|| {
            crate::codex_cli_config_override_value(&args.codex_args, "model_provider").and_then(
                |provider| {
                    prodex_provider_core::provider_implementation_registry()
                        .resolve_model_provider_id(&provider)
                },
            )
        })
        .unwrap_or(prodex_provider_core::ProviderId::OpenAi)
}

pub(super) fn apply_provider_override(
    args: &mut SuperArgs,
    provider: &str,
) -> std::result::Result<(), String> {
    let provider = prodex_provider_core::ProviderId::parse(provider)
        .ok_or_else(|| "provider is unsupported".to_string())?;
    let provider_changed = main_provider(args) != provider;
    // A run-scoped provider override must never reinterpret a key captured for
    // another provider as the new provider's credential.
    if provider_changed {
        args.api_key = None;
        args.local_model = None;
        remove_codex_config_override(&mut args.codex_args, "model");
        remove_codex_config_override(&mut args.codex_args, "model_reasoning_effort");
    }
    remove_codex_config_override(&mut args.codex_args, "model_provider");
    match provider {
        prodex_provider_core::ProviderId::OpenAi => {
            args.provider = None;
            args.url = None;
            args.codex_args.extend([
                OsString::from("-c"),
                OsString::from("model_provider=\"openai\""),
            ]);
        }
        prodex_provider_core::ProviderId::Local => {
            if args.url.is_none() {
                return Err("local provider requires the expose local URL".to_string());
            }
            args.provider = None;
        }
        provider => {
            args.url = None;
            args.provider = prodex_cli::SuperExternalProvider::from_provider_id(provider);
            if args.provider.is_none() {
                return Err("provider is unsupported".to_string());
            }
        }
    }
    Ok(())
}

fn remove_codex_config_override(args: &mut Vec<OsString>, key: &str) {
    let mut retained = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].to_string_lossy();
        if matches!(argument.as_ref(), "-c" | "--config")
            && args
                .get(index + 1)
                .and_then(|value| value.to_str())
                .is_some_and(|assignment| config_assignment_has_key(assignment, key))
        {
            index += 2;
            continue;
        }
        if (argument.starts_with("--config=") || argument.starts_with("-c"))
            && config_assignment_has_key(
                argument
                    .strip_prefix("--config=")
                    .or_else(|| argument.strip_prefix("-c"))
                    .unwrap_or_default(),
                key,
            )
        {
            index += 1;
            continue;
        }
        retained.push(args[index].clone());
        index += 1;
    }
    *args = retained;
}

fn config_assignment_has_key(assignment: &str, key: &str) -> bool {
    assignment
        .split_once('=')
        .is_some_and(|(name, _)| name.trim() == key)
}

pub(super) fn mcp_tool_names() -> [&'static str; 10] {
    [
        "prodex_super_start",
        "prodex_super_status",
        "prodex_super_events",
        "prodex_super_result",
        "prodex_super_cancel",
        "prodex_super_list",
        "prodex_super_exec",
        "prodex_session_prompt_write",
        "prodex_session_preempt",
        "prodex_session_output_read",
    ]
}

pub(super) fn mcp_tools() -> Vec<Value> {
    vec![
        tool_definition(
            "prodex_super_start",
            "Start one full-access Prodex Super task in the captured initial working directory. The task retains normal OS-user filesystem, process, network, Git, and tool authority; the initial directory is not a jail. Use only for explicit user-requested development work; poll its run_id instead of starting duplicates.",
            json!({
                "type": "object",
                "properties": {
                    "task": {"type": "string", "minLength": 1, "maxLength": 65536, "description": "Explicit development task to execute. It starts in the captured directory but may use any filesystem path, executable, network, repository, or local tool available to the Prodex OS user."},
                    "model": {"type": ["string", "null"], "maxLength": 256, "description": "Optional main-agent model override for this run only; null inherits the expose default."},
                    "reasoning_effort": {"type": ["string", "null"], "maxLength": 256, "description": "Optional model-aware main-agent effort override for this run only; null inherits the expose default."},
                    "provider": {"type": ["string", "null"], "maxLength": 256, "description": "Optional main-agent provider override for this run only."},
                    "profile": {"type": ["string", "null"], "maxLength": 128, "description": "Optional Prodex profile override for this run only."},
                    "sub_agents": {"type": ["boolean", "null"]}
                },
                "required": ["task"],
                "additionalProperties": false
            }),
            json!({"type": "object", "properties": {"run_id": {"type": "string"}, "state": {"type": "string"}}, "required": ["run_id", "state"]}),
            false,
            true,
            true,
        ),
        tool_definition(
            "prodex_super_status",
            "Read the current bounded state of one Prodex Super run; always provide its explicit run_id and poll this instead of starting a duplicate.",
            run_id_schema(),
            status_schema(),
            true,
            false,
            false,
        ),
        tool_definition(
            "prodex_super_events",
            "Read a bounded monotonic page of redacted stdout/stderr lifecycle events for one explicit run_id.",
            json!({"type": "object", "properties": {"run_id": {"type": "string"}, "after_seq": {"type": "integer", "minimum": 0}, "limit": {"type": "integer", "minimum": 1, "maximum": MCP_MAX_EVENT_PAGE}}, "required": ["run_id"], "additionalProperties": false}),
            json!({"type": "object", "properties": {"instance_id": {"type": "string"}, "run_id": {"type": "string"}, "events": {"type": "array"}, "next_seq": {"type": "integer"}, "truncated": {"type": "boolean"}}, "required": ["instance_id", "run_id", "events", "next_seq", "truncated"]}),
            true,
            false,
            false,
        ),
        tool_definition(
            "prodex_super_result",
            "Read a bounded final result for one explicit Prodex Super run, or its current nonterminal state.",
            run_id_schema(),
            json!({"type": "object", "properties": {"instance_id": {"type": "string"}, "run_id": {"type": "string"}, "state": {"type": "string"}, "output": {"type": "string"}, "output_truncated": {"type": "boolean"}}, "required": ["instance_id", "run_id", "state"]}),
            true,
            false,
            false,
        ),
        tool_definition(
            "prodex_super_cancel",
            "Cancel one Prodex Super run and terminate only its child process tree.",
            run_id_schema(),
            status_schema(),
            false,
            true,
            false,
        ),
        tool_definition(
            "prodex_super_list",
            "List bounded runs owned by this expose instance; it never lists runs from another instance.",
            json!({"type": "object", "properties": {}, "additionalProperties": false}),
            json!({"type": "object", "properties": {"instance_id": {"type": "string"}, "runs": {"type": "array"}}, "required": ["instance_id", "runs"]}),
            true,
            false,
            false,
        ),
        tool_definition(
            "prodex_super_exec",
            "Execute one direct OS command under the expose process's current local OS-user authority. It is synchronous and standalone: it does not require, create, attach to, or depend on a Prodex Super run or plain prodex s session. Use an explicit shell executable such as sh or cmd.exe when shell syntax is needed.",
            json!({
                "type": "object",
                "properties": {
                    "program": {"type": "string", "minLength": 1, "maxLength": EXEC_MAX_PROGRAM_BYTES},
                    "args": {"type": ["array", "null"], "maxItems": EXEC_MAX_ARGUMENTS, "description": "Direct argv values; shell parsing is not performed.", "items": {"type": "string", "maxLength": EXEC_MAX_ARGUMENT_BYTES}},
                    "cwd": {"type": ["string", "null"], "maxLength": EXEC_MAX_CWD_BYTES, "description": "Working directory; null uses the captured expose workspace."},
                    "env": {"type": ["object", "null"], "maxProperties": EXEC_MAX_ENV_ENTRIES, "additionalProperties": {"type": "string", "maxLength": EXEC_MAX_ENV_VALUE_BYTES}, "propertyNames": {"maxLength": EXEC_MAX_ENV_KEY_BYTES}},
                    "stdin": {"type": ["string", "null"], "maxLength": EXEC_MAX_STDIN_BYTES},
                    "timeout_ms": {"type": ["integer", "null"], "minimum": 1, "maximum": EXEC_MAX_TIMEOUT_MS, "default": EXEC_DEFAULT_TIMEOUT_MS}
                },
                "required": ["program"],
                "additionalProperties": false,
                "x-maxTotalArgumentBytes": EXEC_MAX_ARGUMENTS_BYTES
            }),
            json!({
                "type": "object",
                "properties": {
                    "status": {"type": "string", "enum": ["completed", "timed_out", "cancelled"]},
                    "program": {"type": "string"},
                    "arg_count": {"type": "integer"},
                    "cwd": {"type": "string"},
                    "pid": {"type": ["integer", "null"]},
                    "exit_code": {"type": ["integer", "null"]},
                    "exit_status": {"type": "integer"},
                    "signal": {"type": ["integer", "null"]},
                    "termination": {"type": ["string", "null"]},
                    "success": {"type": "boolean"},
                    "duration_ms": {"type": "integer"},
                    "stdout": {"type": "string", "maxLength": EXEC_MAX_OUTPUT_BYTES},
                    "stdout_truncated": {"type": "boolean"},
                    "stderr": {"type": "string", "maxLength": EXEC_MAX_OUTPUT_BYTES},
                    "stderr_truncated": {"type": "boolean"}
                },
                "required": ["status", "program", "arg_count", "cwd", "pid", "exit_code", "exit_status", "signal", "termination", "success", "duration_ms", "stdout", "stdout_truncated", "stderr", "stderr_truncated"]
            }),
            false,
            true,
            true,
        ),
        tool_definition(
            "prodex_session_prompt_write",
            "Prompt Write: deliver one session input to an already-running plain `prodex s` through the supported Codex control plane. It uses the same fail-closed identity checks as output reads and never starts another solver. A returned output_cursor is an optional pre-write rollout anchor; write_ambiguous means delivery may have happened and must not be replayed.",
            json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string", "minLength": 1, "maxLength": 65536},
                    "cwd": {"type": ["string", "null"], "maxLength": 4096},
                    "prodex_pid": {"type": ["integer", "null"], "minimum": 1},
                    "thread_id": {"type": ["string", "null"], "maxLength": 128}
                },
                "required": ["message"],
                "additionalProperties": false
            }),
            json!({"type": "object", "properties": {"status": {"type": "string"}, "prodex_pid": {"type": "integer"}, "codex_pid": {"type": "integer"}, "thread_id": {"type": "string"}, "message_id": {"type": ["string", "null"]}, "submission_id": {"type": ["string", "null"]}, "output_cursor": {"type": ["string", "null"]}, "queue_exit": {"type": "integer"}, "verification": {"type": "string"}, "recovery_generation": {"type": "integer"}, "last_prompt_requeued": {"type": "boolean"}, "requeue_reason": {"type": ["string", "null"]}}, "required": ["status", "prodex_pid", "codex_pid", "thread_id", "message_id", "submission_id", "output_cursor", "queue_exit", "verification", "recovery_generation", "last_prompt_requeued", "requeue_reason"]}),
            false,
            false,
            false,
        ),
        tool_definition(
            "prodex_session_preempt",
            "Preempt the current turn for one exact existing plain `prodex s` session, then remove every still-pending queued prompt through Codex. It never kills a process or reports an already-started prompt as cancelled; ambiguous queue or interrupt state fails closed.",
            json!({
                "type": "object",
                "properties": {
                    "cwd": {"type": ["string", "null"], "maxLength": 4096},
                    "prodex_pid": {"type": ["integer", "null"], "minimum": 1},
                    "thread_id": {"type": ["string", "null"], "maxLength": 128}
                },
                "additionalProperties": false
            }),
            json!({"type": "object", "properties": {"status": {"type": "string"}, "preempted": {"type": "boolean"}, "prodex_pid": {"type": "integer"}, "codex_pid": {"type": "integer"}, "thread_id": {"type": "string"}, "current_turn_id": {"type": ["string", "null"]}, "current_turn_found": {"type": "boolean"}, "current_turn_interrupted": {"type": "boolean"}, "cancelled_submission_ids": {"type": "array", "items": {"type": "string"}}, "cancelled_count": {"type": "integer"}, "remaining_submission_ids": {"type": "array", "items": {"type": "string"}}, "remaining_count": {"type": "integer"}, "queue_empty_at_boundary": {"type": "boolean"}, "session_ready": {"type": "boolean"}, "generation": {"type": "integer"}, "generation_boundary": {"type": "integer"}}, "required": ["status", "preempted", "prodex_pid", "codex_pid", "thread_id", "current_turn_id", "current_turn_found", "current_turn_interrupted", "cancelled_submission_ids", "cancelled_count", "remaining_submission_ids", "remaining_count", "queue_empty_at_boundary", "session_ready", "generation", "generation_boundary"]}),
            false,
            true,
            false,
        ),
        tool_definition(
            "prodex_session_output_read",
            "Read bounded user-visible output from the same already-running plain `prodex s` interactive session; it never starts another solver or reads its PTY. Pages may contain generic gap markers for safely skipped malformed records and text_truncated markers for bounded text.",
            json!({
                "type": "object",
                "properties": {
                    "cursor": {"type": ["string", "null"], "maxLength": 16384},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 200},
                    "wait_ms": {"type": "integer", "minimum": 0, "maximum": 10000},
                    "prodex_pid": {"type": ["integer", "null"], "minimum": 1},
                    "thread_id": {"type": ["string", "null"], "maxLength": 128}
                },
                "additionalProperties": false
            }),
            json!({"type": "object", "properties": {"status": {"type": "string"}, "prodex_pid": {"type": "integer"}, "codex_pid": {"type": "integer"}, "thread_id": {"type": "string"}, "source": {"type": "string"}, "events": {"type": "array"}, "next_cursor": {"type": "string"}, "has_more": {"type": "boolean"}}, "required": ["status", "prodex_pid", "codex_pid", "thread_id", "source", "events", "next_cursor", "has_more"]}),
            true,
            false,
            false,
        ),
    ]
}

fn run_id_schema() -> Value {
    json!({"type": "object", "properties": {"run_id": {"type": "string", "minLength": 1, "maxLength": EXPOSE_MAX_RUN_ID_BYTES}}, "required": ["run_id"], "additionalProperties": false})
}

fn status_schema() -> Value {
    json!({"type": "object", "properties": {"instance_id": {"type": "string"}, "run_id": {"type": "string"}, "state": {"type": "string"}, "created_at": {"type": "integer"}, "started_at": {"type": ["integer", "null"]}, "finished_at": {"type": ["integer", "null"]}, "exit_status": {"type": ["integer", "null"]}, "provider": {"type": ["string", "null"]}, "model": {"type": ["string", "null"]}, "reasoning_effort": {"type": ["string", "null"]}, "cancellation_requested": {"type": "boolean"}}, "required": ["instance_id", "run_id", "state"]})
}

fn tool_definition(
    name: &str,
    description: &str,
    input_schema: Value,
    output_schema: Value,
    read_only: bool,
    destructive: bool,
    open_world: bool,
) -> Value {
    json!({
        "name": name,
        "title": name.replace('_', " "),
        "description": description,
        "inputSchema": input_schema,
        "outputSchema": output_schema,
        "annotations": {"readOnlyHint": read_only, "destructiveHint": destructive, "openWorldHint": open_world},
    })
}

pub(super) fn run_summary_json(summary: &ExposeRunSummary) -> Value {
    json!({
        "run_id": summary.run_id,
        "state": summary.state.as_str(),
        "created_at": summary.created_at,
        "started_at": summary.started_at,
        "finished_at": summary.finished_at,
        "exit_status": summary.exit_status,
        "provider": summary.provider,
        "model": summary.model,
        "reasoning_effort": summary.reasoning_effort,
        "cancellation_requested": summary.cancellation_requested,
    })
}
