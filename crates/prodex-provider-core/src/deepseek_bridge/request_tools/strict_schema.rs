//! DeepSeek strict function-tool schema sanitizer.

pub(super) fn deepseek_provider_core_sanitize_strict_schema(
    schema: &mut serde_json::Value,
    path: &str,
    provider_label: &str,
) -> Result<(), String> {
    let Some(object) = schema.as_object_mut() else {
        return Err(format!(
            "{provider_label} strict tool schema `{path}` must be a JSON object"
        ));
    };
    deepseek_provider_core_reject_strict_schema_keywords(object, path, provider_label)?;
    if let Some(any_of) = object.get_mut("anyOf") {
        let Some(items) = any_of.as_array_mut() else {
            return Err(format!(
                "{provider_label} strict tool schema `{path}.anyOf` must be an array"
            ));
        };
        for (index, item) in items.iter_mut().enumerate() {
            deepseek_provider_core_sanitize_strict_schema(
                item,
                &format!("{path}.anyOf[{index}]"),
                provider_label,
            )?;
        }
        return Ok(());
    }
    if let Some(enum_values) = object.get("enum")
        && !enum_values.is_array()
    {
        return Err(format!(
            "{provider_label} strict tool schema `{path}.enum` must be an array"
        ));
    }
    let schema_type = object
        .entry("type".to_string())
        .or_insert_with(|| serde_json::Value::String("object".to_string()))
        .as_str()
        .ok_or_else(|| {
            format!("{provider_label} strict tool schema `{path}.type` must be a string")
        })?;
    if !matches!(
        schema_type,
        "object" | "string" | "number" | "integer" | "boolean" | "array"
    ) {
        return Err(format!(
            "{provider_label} strict tool schema `{path}` uses unsupported type `{schema_type}`"
        ));
    }
    match schema_type {
        "object" => {
            deepseek_provider_core_sanitize_strict_object_schema(object, path, provider_label)
        }
        "array" => {
            let Some(items) = object.get_mut("items") else {
                return Err(format!(
                    "{provider_label} strict tool schema `{path}` array requires items"
                ));
            };
            deepseek_provider_core_sanitize_strict_schema(
                items,
                &format!("{path}.items"),
                provider_label,
            )
        }
        _ => Ok(()),
    }
}

fn deepseek_provider_core_sanitize_strict_object_schema(
    object: &mut serde_json::Map<String, serde_json::Value>,
    path: &str,
    provider_label: &str,
) -> Result<(), String> {
    let properties = object
        .entry("properties".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let Some(properties) = properties.as_object_mut() else {
        return Err(format!(
            "{provider_label} strict tool schema `{path}.properties` must be an object"
        ));
    };
    let required = properties.keys().cloned().collect::<Vec<_>>();
    for (name, property) in properties.iter_mut() {
        deepseek_provider_core_sanitize_strict_schema(
            property,
            &format!("{path}.{name}"),
            provider_label,
        )?;
    }
    object.insert("required".to_string(), serde_json::json!(required));
    object.insert(
        "additionalProperties".to_string(),
        serde_json::Value::Bool(false),
    );
    Ok(())
}

fn deepseek_provider_core_reject_strict_schema_keywords(
    object: &serde_json::Map<String, serde_json::Value>,
    path: &str,
    provider_label: &str,
) -> Result<(), String> {
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "type"
                | "description"
                | "properties"
                | "required"
                | "additionalProperties"
                | "items"
                | "enum"
                | "anyOf"
        ) {
            return Err(format!(
                "{provider_label} strict tool schema `{path}` uses unsupported keyword `{key}`"
            ));
        }
    }
    Ok(())
}
