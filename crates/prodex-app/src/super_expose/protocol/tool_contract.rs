use super::*;

pub(super) fn validate_tool_arguments(
    tool: &str,
    arguments: &Value,
) -> std::result::Result<(), String> {
    let Some(object) = arguments.as_object() else {
        return Err("tool arguments must be an object".to_string());
    };
    if let Some(unknown) = object.keys().find(|key| {
        !prodex_mojo_core::rich::super_expose_tool_argument_allowed(tool, key)
            .expect("Mojo Super expose tool-argument policy returned invalid output")
    }) {
        return Err(format!("unknown tool argument: {unknown}"));
    }
    Ok(())
}

#[derive(serde::Deserialize)]
struct CompiledToolCatalog {
    tools: Vec<Value>,
    exec_index: usize,
}

pub(super) fn tools(mode: SuperExposeMode, optional_tools: &ExposeOptionalTools) -> Vec<Value> {
    let available_ids = json!(optional_tools.available_ids());
    let source = crate::json_boundary::app_json_tree(&available_ids)
        .expect("fixed optional-tool identifiers fit the JSON ABI");
    let body = prodex_mojo_core::json::super_expose_tool_catalog(
        &source.nodes,
        std::str::from_utf8(&source.raw).expect("Serde emits UTF-8 JSON"),
        mode.exec_only(),
        MCP_MAX_EVENT_PAGE,
    )
    .expect("compiled Mojo MCP tool catalog failed");
    let mut catalog: CompiledToolCatalog = serde_json::from_slice(&body)
        .expect("compiled Mojo MCP tool catalog returned invalid JSON");
    let exec = catalog
        .tools
        .get_mut(catalog.exec_index)
        .filter(|tool| tool["name"] == "prodex_super_exec")
        .expect("compiled Mojo MCP tool catalog returned an invalid exec index");
    // Operational paths and status details remain in Rust's endpoint boundary.
    exec["_meta"] = json!({"prodex/optionalTools": optional_tools.public_manifest()});
    catalog.tools
}

pub(super) fn required_run_id(arguments: &Value) -> std::result::Result<String, String> {
    let value = required_string(arguments, "run_id", 128)?;
    if prodex_mojo_core::rich::super_expose_run_id_valid(&value)
        .expect("Mojo Super expose run-id policy returned invalid output")
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
    if !prodex_mojo_core::rich::super_expose_string_valid(value, max_bytes)
        .expect("Mojo Super expose optional-string policy returned invalid output")
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

#[cfg(test)]
#[path = "tool_catalog_tests.rs"]
mod catalog_tests;
