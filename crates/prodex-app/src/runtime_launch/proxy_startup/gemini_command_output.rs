pub(super) fn runtime_gemini_structured_command_tool_response(
    tool_name: &str,
    output: &str,
) -> Option<serde_json::Value> {
    if !runtime_gemini_is_command_tool_name(tool_name) || output.trim().is_empty() {
        return None;
    }
    if let Some(session_id) =
        runtime_gemini_u64_after_marker(output, "Process running with session ID")
    {
        return Some(serde_json::json!({
            "output": output,
            "status": "running",
            "codex_tool_status": "running",
            "running_session_id": session_id,
            "next_required_action": format!(
                "Call write_stdin with session_id={session_id} until the process exits or yields the needed output."
            ),
        }));
    }
    if let Some(exit_code) = runtime_gemini_i64_after_marker(output, "Process exited with code") {
        let mut response = serde_json::json!({
            "output": output,
            "status": if exit_code == 0 { "exited" } else { "failed" },
            "codex_tool_status": if exit_code == 0 { "exited" } else { "failed" },
            "exit_code": exit_code,
        });
        if exit_code != 0 {
            runtime_gemini_add_command_failure_guidance(&mut response, output);
        }
        return Some(response);
    }
    None
}

fn runtime_gemini_is_command_tool_name(tool_name: &str) -> bool {
    let normalized = tool_name
        .rsplit(['.', ':'])
        .next()
        .unwrap_or(tool_name)
        .rsplit("__")
        .next()
        .unwrap_or(tool_name);
    matches!(normalized, "exec_command" | "write_stdin" | "shell")
}

fn runtime_gemini_u64_after_marker(text: &str, marker: &str) -> Option<u64> {
    let tail = text.split_once(marker)?.1.trim_start();
    let digits = tail
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse::<u64>().ok())?
}

fn runtime_gemini_i64_after_marker(text: &str, marker: &str) -> Option<i64> {
    let tail = text.split_once(marker)?.1.trim_start();
    let mut value = String::new();
    for (index, ch) in tail.chars().enumerate() {
        if ch.is_ascii_digit() || (index == 0 && ch == '-') {
            value.push(ch);
            continue;
        }
        break;
    }
    if value == "-" || value.is_empty() {
        return None;
    }
    value.parse::<i64>().ok()
}

fn runtime_gemini_add_command_failure_guidance(response: &mut serde_json::Value, output: &str) {
    let lower = output.to_ascii_lowercase();
    response["error"] = serde_json::Value::Bool(true);
    response["failure_kind"] = serde_json::Value::String(
        if lower.contains("no such file or directory") || lower.contains("cannot access") {
            "missing_path"
        } else if lower.contains("git clone")
            || lower.contains("cloning into")
            || lower.contains("error reading section header")
            || lower.contains("the requested url returned error")
            || lower.contains("repository not found")
        {
            "clone_or_network"
        } else if lower.contains("command not found") {
            "missing_command"
        } else {
            "command_failed"
        }
        .to_string(),
    );
    response["next_required_action"] = serde_json::Value::String(
        "Treat this command as failed. Do not use files or paths this command was supposed to create until a follow-up command verifies they exist. Inspect the exact error, fix the root cause or rerun a corrected command, then verify before continuing."
            .to_string(),
    );
    if let Some(path) = runtime_gemini_extract_missing_path(output)
        .or_else(|| runtime_gemini_extract_clone_target(output))
    {
        response["affected_path"] = serde_json::Value::String(path);
        response["path_verified"] = serde_json::Value::Bool(false);
    }
}

fn runtime_gemini_extract_missing_path(output: &str) -> Option<String> {
    runtime_gemini_quoted_value_after_marker(output, "cannot access")
        .or_else(|| runtime_gemini_quoted_value_after_marker(output, "No such file or directory"))
}

fn runtime_gemini_extract_clone_target(output: &str) -> Option<String> {
    runtime_gemini_quoted_value_after_marker(output, "Cloning into")
}

fn runtime_gemini_quoted_value_after_marker(output: &str, marker: &str) -> Option<String> {
    let tail = output.split_once(marker)?.1;
    let start = tail.find('\'')? + 1;
    let tail = &tail[start..];
    let end = tail.find('\'')?;
    let value = tail[..end].trim();
    (!value.is_empty()).then(|| value.to_string())
}
