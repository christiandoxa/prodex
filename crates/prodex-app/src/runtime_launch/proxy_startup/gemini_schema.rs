pub(super) fn runtime_gemini_sanitize_function_schema(
    schema: &serde_json::Value,
) -> serde_json::Value {
    let Some(object) = schema.as_object() else {
        return serde_json::json!({ "type": "object" });
    };

    if let Some(collapsed) = runtime_gemini_collapse_schema_union(object) {
        return collapsed;
    }

    let mut sanitized = serde_json::Map::new();
    let mut nullable = object
        .get("nullable")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if let Some(schema_type) = runtime_gemini_schema_type(object, &mut nullable) {
        sanitized.insert(
            "type".to_string(),
            serde_json::Value::String(schema_type.to_string()),
        );
    }
    if nullable {
        sanitized.insert("nullable".to_string(), serde_json::Value::Bool(true));
    }
    for key in ["description", "format"] {
        if let Some(value) = object.get(key).and_then(serde_json::Value::as_str)
            && !value.trim().is_empty()
        {
            sanitized.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }
    if let Some(enum_values) = runtime_gemini_sanitized_enum(object.get("enum")) {
        sanitized.insert("enum".to_string(), serde_json::Value::Array(enum_values));
    }
    if let Some(properties) = runtime_gemini_sanitized_properties(object.get("properties")) {
        sanitized.insert("properties".to_string(), properties);
    }
    if let Some(required) = runtime_gemini_sanitized_required(object.get("required")) {
        sanitized.insert("required".to_string(), required);
    }
    if let Some(items) = object.get("items") {
        sanitized.insert(
            "items".to_string(),
            runtime_gemini_sanitize_function_schema(items),
        );
    }
    if !sanitized.contains_key("type") {
        sanitized.insert(
            "type".to_string(),
            serde_json::Value::String("object".to_string()),
        );
    }
    serde_json::Value::Object(sanitized)
}

fn runtime_gemini_collapse_schema_union(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<serde_json::Value> {
    let alternatives = object
        .get("anyOf")
        .or_else(|| object.get("oneOf"))
        .and_then(serde_json::Value::as_array)?;
    let non_null = alternatives
        .iter()
        .filter(|value| {
            value
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_none_or(|schema_type| schema_type != "null")
        })
        .collect::<Vec<_>>();
    if non_null.len() != 1 {
        return None;
    }
    let mut sanitized = runtime_gemini_sanitize_function_schema(non_null[0]);
    if alternatives.len() != non_null.len()
        && let Some(sanitized_object) = sanitized.as_object_mut()
    {
        sanitized_object.insert("nullable".to_string(), serde_json::Value::Bool(true));
    }
    Some(sanitized)
}

fn runtime_gemini_schema_type<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    nullable: &mut bool,
) -> Option<&'a str> {
    match object.get("type") {
        Some(serde_json::Value::String(schema_type)) => {
            runtime_gemini_supported_schema_type(schema_type)
        }
        Some(serde_json::Value::Array(types)) => {
            let mut selected = None;
            for schema_type in types.iter().filter_map(serde_json::Value::as_str) {
                if schema_type == "null" {
                    *nullable = true;
                } else if selected.is_none() {
                    selected = runtime_gemini_supported_schema_type(schema_type);
                }
            }
            selected
        }
        _ if object.contains_key("properties") => Some("object"),
        _ if object.contains_key("items") => Some("array"),
        _ if object.contains_key("enum") => Some("string"),
        _ => None,
    }
}

fn runtime_gemini_supported_schema_type(schema_type: &str) -> Option<&'static str> {
    match schema_type.to_ascii_lowercase().as_str() {
        "object" => Some("object"),
        "array" => Some("array"),
        "string" => Some("string"),
        "integer" => Some("integer"),
        "number" => Some("number"),
        "boolean" => Some("boolean"),
        _ => None,
    }
}

fn runtime_gemini_sanitized_enum(
    value: Option<&serde_json::Value>,
) -> Option<Vec<serde_json::Value>> {
    let values = value?.as_array()?;
    let strings = values
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(|value| serde_json::Value::String(value.to_string()))
        .collect::<Vec<_>>();
    (!strings.is_empty()).then_some(strings)
}

fn runtime_gemini_sanitized_properties(
    value: Option<&serde_json::Value>,
) -> Option<serde_json::Value> {
    let properties = value?.as_object()?;
    let sanitized = properties
        .iter()
        .map(|(name, schema)| {
            (
                name.clone(),
                runtime_gemini_sanitize_function_schema(schema),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    Some(serde_json::Value::Object(sanitized))
}

fn runtime_gemini_sanitized_required(
    value: Option<&serde_json::Value>,
) -> Option<serde_json::Value> {
    let required = value?.as_array()?;
    let required = required
        .iter()
        .filter_map(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(|name| serde_json::Value::String(name.to_string()))
        .collect::<Vec<_>>();
    (!required.is_empty()).then_some(serde_json::Value::Array(required))
}
