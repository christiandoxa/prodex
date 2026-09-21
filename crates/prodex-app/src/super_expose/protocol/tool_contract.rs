use super::*;

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

pub(super) fn tools(mode: SuperExposeMode) -> Vec<Value> {
    vec![
        tool_definition(
            "prodex_super_start",
            "Start one full-access Prodex Super task in the captured initial working directory. The task retains normal OS-user filesystem, process, network, Git, and local-tool authority; the initial directory is not a jail. Use only for explicit user-requested development work; poll its run_id instead of starting duplicates.",
            json!({"type":"object","properties":{"task":{"type":"string","minLength":1,"maxLength":65536},"model":{"type":["string","null"],"maxLength":256},"reasoning_effort":{"type":["string","null"],"maxLength":256},"provider":{"type":["string","null"],"maxLength":256},"profile":{"type":["string","null"],"maxLength":128},"sub_agents":{"type":["boolean","null"]}},"required":["task"],"additionalProperties":false}),
            json!({"type":"object","properties":{"run_id":{"type":"string"},"state":{"type":"string"}},"required":["run_id","state"]}),
            false, true, true,
        ),
        tool_definition(
            "prodex_super_status",
            "Read the current bounded state of one Prodex Super run; always provide its explicit run_id and poll this instead of starting a duplicate.",
            run_id_schema(), status_schema(), true, false, false,
        ),
        tool_definition(
            "prodex_super_events",
            "Read a bounded monotonic page of redacted stdout/stderr lifecycle events for one explicit run_id.",
            json!({"type":"object","properties":{"run_id":{"type":"string"},"after_seq":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":MCP_MAX_EVENT_PAGE}},"required":["run_id"],"additionalProperties":false}),
            json!({"type":"object","properties":{"instance_id":{"type":"string"},"run_id":{"type":"string"},"events":{"type":"array"},"next_seq":{"type":"integer"},"truncated":{"type":"boolean"}},"required":["instance_id","run_id","events","next_seq","truncated"]}),
            true, false, false,
        ),
        tool_definition(
            "prodex_super_result",
            "Read a bounded final result for one explicit Prodex Super run, or its current nonterminal state.",
            run_id_schema(),
            json!({"type":"object","properties":{"instance_id":{"type":"string"},"run_id":{"type":"string"},"state":{"type":"string"},"output":{"type":"string"},"output_truncated":{"type":"boolean"}},"required":["instance_id","run_id","state"]}),
            true, false, false,
        ),
        tool_definition(
            "prodex_super_cancel",
            "Cancel one Prodex Super run and terminate only its child process tree.",
            run_id_schema(), status_schema(), false, true, false,
        ),
        tool_definition(
            "prodex_super_list",
            "List bounded runs owned by this expose instance; it never lists runs from another instance.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            json!({"type":"object","properties":{"instance_id":{"type":"string"},"runs":{"type":"array"}},"required":["instance_id","runs"]}),
            true, false, false,
        ),
        tool_definition(
            "prodex_super_exec",
            "Execute one direct OS command under the expose process's current local OS-user authority. It is synchronous and standalone: it does not require, create, attach to, or depend on a Prodex Super run or plain prodex s session. Use an explicit shell executable such as sh or cmd.exe when shell syntax is needed.",
            json!({"type":"object","properties":{"program":{"type":"string","minLength":1,"maxLength":4096},"args":{"type":["array","null"],"maxItems":256,"items":{"type":"string","maxLength":16384}},"cwd":{"type":["string","null"],"maxLength":4096},"env":{"type":["object","null"],"maxProperties":128,"additionalProperties":{"type":"string","maxLength":16384},"propertyNames":{"maxLength":256}},"stdin":{"type":["string","null"],"maxLength":262144},"timeout_ms":{"type":["integer","null"],"minimum":1,"maximum":120000,"default":30000}},"required":["program"],"additionalProperties":false,"x-maxTotalArgumentBytes":262144}),
            json!({"type":"object","properties":{"status":{"type":"string","enum":["completed","timed_out","cancelled"]},"program":{"type":"string"},"arg_count":{"type":"integer"},"cwd":{"type":"string"},"pid":{"type":["integer","null"]},"exit_code":{"type":["integer","null"]},"exit_status":{"type":"integer"},"signal":{"type":["integer","null"]},"termination":{"type":["string","null"]},"success":{"type":"boolean"},"duration_ms":{"type":"integer"},"stdout":{"type":"string","maxLength":131072},"stdout_truncated":{"type":"boolean"},"stderr":{"type":"string","maxLength":131072},"stderr_truncated":{"type":"boolean"}},"required":["status","program","arg_count","cwd","pid","exit_code","exit_status","signal","termination","success","duration_ms","stdout","stdout_truncated","stderr","stderr_truncated"]}),
            false, true, true,
        ),
        tool_definition(
            "prodex_session_prompt_write",
            "Prompt Write: deliver one session input to an already-running plain prodex s through the supported Codex control plane. It uses the same fail-closed identity checks as output reads and never starts another solver. A returned output_cursor is an optional pre-write rollout anchor; write_ambiguous means delivery may have happened and must not be replayed.",
            json!({"type":"object","properties":{"message":{"type":"string","minLength":1,"maxLength":65536},"cwd":{"type":["string","null"],"maxLength":4096},"prodex_pid":{"type":["integer","null"],"minimum":1},"thread_id":{"type":["string","null"],"maxLength":128}},"required":["message"],"additionalProperties":false}),
            json!({"type":"object","properties":{"status":{"type":"string"},"prodex_pid":{"type":"integer"},"codex_pid":{"type":"integer"},"thread_id":{"type":"string"},"message_id":{"type":["string","null"]},"submission_id":{"type":["string","null"]},"output_cursor":{"type":["string","null"]},"queue_exit":{"type":"integer"},"verification":{"type":"string"},"recovery_generation":{"type":"integer"},"last_prompt_requeued":{"type":"boolean"},"requeue_reason":{"type":["string","null"]}},"required":["status","prodex_pid","codex_pid","thread_id","message_id","submission_id","output_cursor","queue_exit","verification","recovery_generation","last_prompt_requeued","requeue_reason"]}),
            false, false, false,
        ),
        tool_definition(
            "prodex_session_preempt",
            "Preempt the current turn for one exact existing plain prodex s session, then remove every still-pending queued prompt through Codex. It never kills a process or reports an already-started prompt as cancelled; ambiguous queue or interrupt state fails closed.",
            json!({"type":"object","properties":{"cwd":{"type":["string","null"],"maxLength":4096},"prodex_pid":{"type":["integer","null"],"minimum":1},"thread_id":{"type":["string","null"],"maxLength":128}},"additionalProperties":false}),
            json!({"type":"object","properties":{"status":{"type":"string"},"preempted":{"type":"boolean"},"prodex_pid":{"type":"integer"},"codex_pid":{"type":"integer"},"thread_id":{"type":"string"},"current_turn_id":{"type":["string","null"]},"current_turn_found":{"type":"boolean"},"current_turn_interrupted":{"type":"boolean"},"cancelled_submission_ids":{"type":"array","items":{"type":"string"}},"cancelled_count":{"type":"integer"},"remaining_submission_ids":{"type":"array","items":{"type":"string"}},"remaining_count":{"type":"integer"},"queue_empty_at_boundary":{"type":"boolean"},"session_ready":{"type":"boolean"},"generation":{"type":"integer"},"generation_boundary":{"type":"integer"}},"required":["status","preempted","prodex_pid","codex_pid","thread_id","current_turn_id","current_turn_found","current_turn_interrupted","cancelled_submission_ids","cancelled_count","remaining_submission_ids","remaining_count","queue_empty_at_boundary","session_ready","generation","generation_boundary"]}),
            false, true, false,
        ),
        tool_definition(
            "prodex_session_output_read",
            "Read bounded user-visible output from the same already-running plain prodex s interactive session; it never starts another solver or reads its PTY. Pages may contain generic gap markers for safely skipped malformed records and text_truncated markers for bounded text.",
            json!({"type":"object","properties":{"cursor":{"type":["string","null"],"maxLength":16384},"limit":{"type":"integer","minimum":1,"maximum":200},"wait_ms":{"type":"integer","minimum":0,"maximum":10000},"prodex_pid":{"type":["integer","null"],"minimum":1},"thread_id":{"type":["string","null"],"maxLength":128}},"additionalProperties":false}),
            json!({"type":"object","properties":{"status":{"type":"string"},"prodex_pid":{"type":"integer"},"codex_pid":{"type":"integer"},"thread_id":{"type":"string"},"source":{"type":"string"},"events":{"type":"array"},"next_cursor":{"type":"string"},"has_more":{"type":"boolean"}},"required":["status","prodex_pid","codex_pid","thread_id","source","events","next_cursor","has_more"]}),
            true, false, false,
        ),
    ]
    .into_iter()
    .filter(|tool| {
        tool.get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| tool_allowed(mode, name))
    })
    .collect()
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
        "annotations": {
            "readOnlyHint": read_only,
            "destructiveHint": destructive,
            "openWorldHint": open_world
        },
    })
}

fn status_schema() -> Value {
    json!({"type":"object","properties":{"instance_id":{"type":"string"},"run_id":{"type":"string"},"state":{"type":"string"},"created_at":{"type":"integer"},"started_at":{"type":["integer","null"]},"finished_at":{"type":["integer","null"]},"exit_status":{"type":["integer","null"]},"provider":{"type":["string","null"]},"model":{"type":["string","null"]},"reasoning_effort":{"type":["string","null"]},"cancellation_requested":{"type":"boolean"}},"required":["instance_id","run_id","state"]})
}

pub(super) fn run_id_schema() -> Value {
    json!({
        "type":"object",
        "properties":{"run_id":{"type":"string","minLength":1,"maxLength":128}},
        "required":["run_id"],
        "additionalProperties":false
    })
}

pub(super) fn required_run_id(arguments: &Value) -> std::result::Result<String, String> {
    let value = required_string(arguments, "run_id", 128)?;
    if value.starts_with("spr_")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        Ok(value)
    } else {
        Err("run_id is invalid".to_string())
    }
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

pub(super) fn value_u64(value: &Value) -> std::result::Result<u64, String> {
    value
        .as_u64()
        .ok_or_else(|| "value must be a non-negative integer".to_string())
}

pub(super) fn value_usize(value: &Value) -> std::result::Result<usize, String> {
    usize::try_from(value_u64(value)?).map_err(|_| "integer is too large".to_string())
}

pub(super) fn optional_process_id(arguments: &Value) -> std::result::Result<Option<u32>, String> {
    let Some(value) = arguments.get("prodex_pid") else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .map(Some)
        .ok_or_else(|| "prodex_pid is invalid".to_string())
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

pub(super) fn normalized_thread_id(
    arguments: &Value,
) -> std::result::Result<Option<String>, String> {
    optional_string(arguments, "thread_id", 128)?.map_or(Ok(None), |thread_id| {
        Uuid::parse_str(&thread_id)
            .map(|thread_id| Some(thread_id.to_string()))
            .map_err(|_| "thread_id is invalid".to_string())
    })
}

pub(super) fn session_binding_key(
    instance_id: &str,
    prodex_pid: Option<u32>,
    thread_id: Option<&str>,
) -> String {
    format!(
        "{instance_id}:pid={}:thread={}",
        prodex_pid.map_or_else(|| "*".to_string(), |pid| pid.to_string()),
        thread_id.unwrap_or("*")
    )
}
