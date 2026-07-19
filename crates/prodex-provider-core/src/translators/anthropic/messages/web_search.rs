use serde_json::{Map, Value, json};

pub(super) fn anthropic_web_search_tool(value: &Value) -> Result<(Value, Option<Value>), String> {
    let Some(options) = value.as_object() else {
        return Err("web_search_options must be an object".to_string());
    };
    for field in options.keys() {
        if !matches!(
            field.as_str(),
            "search_context_size"
                | "allowed_domains"
                | "blocked_domains"
                | "user_location"
                | "max_uses"
        ) {
            return Err(format!(
                "Anthropic Messages does not translate web_search_options field `{field}`"
            ));
        }
    }
    if options.contains_key("allowed_domains") && options.contains_key("blocked_domains") {
        return Err(
            "Anthropic web search cannot combine allowed_domains and blocked_domains".to_string(),
        );
    }
    for field in ["allowed_domains", "blocked_domains"] {
        if let Some(domains) = options.get(field) {
            let Some(domains) = domains.as_array() else {
                return Err(format!("Anthropic web search {field} must be an array"));
            };
            if domains
                .iter()
                .any(|domain| domain.as_str().is_none_or(str::is_empty))
            {
                return Err(format!(
                    "Anthropic web search {field} entries must be non-empty strings"
                ));
            }
        }
    }
    if options
        .get("max_uses")
        .is_some_and(|value| value.as_u64().is_none_or(|value| value == 0))
    {
        return Err("Anthropic web search max_uses must be a positive integer".to_string());
    }
    if options
        .get("user_location")
        .is_some_and(|value| !value.is_object())
    {
        return Err("Anthropic web search user_location must be an object".to_string());
    }
    let ignored_context_size = match options.get("search_context_size") {
        Some(Value::String(value)) if matches!(value.as_str(), "low" | "medium" | "high") => {
            Some(Value::String(value.clone()))
        }
        Some(_) => {
            return Err(
                "Anthropic web search search_context_size must be low, medium, or high".to_string(),
            );
        }
        None => None,
    };
    let mut tool = Map::from_iter([
        (
            "type".to_string(),
            Value::String("web_search_20250305".to_string()),
        ),
        ("name".to_string(), Value::String("web_search".to_string())),
    ]);
    for field in [
        "allowed_domains",
        "blocked_domains",
        "user_location",
        "max_uses",
    ] {
        if let Some(value) = options.get(field) {
            tool.insert(field.to_string(), value.clone());
        }
    }
    Ok((Value::Object(tool), ignored_context_size))
}

pub(super) fn anthropic_web_search_call(block: &Value) -> Result<Value, String> {
    let Some(id) = block.get("id").and_then(Value::as_str) else {
        return Err("Anthropic server_tool_use block must contain id".to_string());
    };
    if block.get("name").and_then(Value::as_str) != Some("web_search") {
        return Err("unsupported Anthropic server tool".to_string());
    }
    let queries = anthropic_web_search_queries(block.get("input"));
    Ok(json!({
        "type": "web_search_call",
        "id": id,
        "status": "completed",
        "action": {
            "type": "search",
            "queries": queries,
            "sources": [],
        },
    }))
}

fn anthropic_web_search_queries(input: Option<&Value>) -> Vec<Value> {
    if let Some(query) = input
        .and_then(|input| input.get("query"))
        .and_then(Value::as_str)
    {
        return vec![Value::String(query.to_string())];
    }
    input
        .and_then(|input| input.get("queries"))
        .and_then(Value::as_array)
        .map(|queries| {
            queries
                .iter()
                .filter_map(Value::as_str)
                .map(|query| Value::String(query.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn merge_anthropic_web_search_result(output: &mut [Value], block: &Value) {
    let Some(tool_use_id) = block.get("tool_use_id").and_then(Value::as_str) else {
        return;
    };
    let sources = anthropic_web_search_sources(block);
    let Some(call) = output.iter_mut().rev().find(|item| {
        item.get("type").and_then(Value::as_str) == Some("web_search_call")
            && item.get("id").and_then(Value::as_str) == Some(tool_use_id)
    }) else {
        return;
    };
    call["action"]["sources"] = Value::Array(sources);
}

fn anthropic_web_search_sources(block: &Value) -> Vec<Value> {
    block
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|result| {
            let url = result.get("url").and_then(Value::as_str)?;
            let mut source = json!({"type": "url", "url": url});
            if let Some(title) = result.get("title").and_then(Value::as_str) {
                source["title"] = Value::String(title.to_string());
            }
            Some(source)
        })
        .collect()
}

pub(super) fn anthropic_tool_usage(value: Option<&Value>) -> Option<Value> {
    let requests = value?
        .pointer("/server_tool_use/web_search_requests")?
        .as_u64()?;
    Some(json!({"web_search": {"num_requests": requests}}))
}
