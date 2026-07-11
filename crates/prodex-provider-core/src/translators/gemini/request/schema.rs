//! Gemini schema sanitization for request tools and response formats.

#[path = "schema/composition.rs"]
mod composition;

use self::composition::{collapse_schema_union, has_composition, sanitized_composition};
use serde_json::{Value, json};

pub(crate) fn sanitize_schema(schema: &Value) -> Value {
    match schema {
        Value::Array(values) => Value::Array(values.iter().map(sanitize_schema).collect()),
        Value::Object(map) => {
            let mut next = serde_json::Map::new();
            for (key, value) in map {
                if matches!(key.as_str(), "strict" | "$schema" | "additionalProperties") {
                    continue;
                }
                next.insert(key.clone(), sanitize_schema(value));
            }
            Value::Object(next)
        }
        _ => schema.clone(),
    }
}

pub(crate) fn sanitize_function_schema(schema: &Value) -> Value {
    let Some(object) = schema.as_object() else {
        return json!({ "type": "object" });
    };

    if let Some(collapsed) = collapse_schema_union(object) {
        return collapsed;
    }

    let mut sanitized = serde_json::Map::new();
    let mut nullable = object
        .get("nullable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if let Some(schema_type) = schema_type(object, &mut nullable) {
        sanitized.insert("type".to_string(), Value::String(schema_type.to_string()));
    }
    if nullable {
        sanitized.insert("nullable".to_string(), Value::Bool(true));
    }
    for key in ["description", "format"] {
        if let Some(value) = object.get(key).and_then(Value::as_str)
            && !value.trim().is_empty()
        {
            sanitized.insert(key.to_string(), Value::String(value.to_string()));
        }
    }
    if let Some(enum_values) = sanitized_enum(object.get("enum")) {
        sanitized.insert("enum".to_string(), Value::Array(enum_values));
    }
    if let Some(properties) = sanitized_properties(object.get("properties")) {
        sanitized.insert("properties".to_string(), properties);
    }
    if let Some(required) = sanitized_required(object.get("required")) {
        sanitized.insert("required".to_string(), required);
    }
    if let Some(items) = object.get("items") {
        sanitized.insert("items".to_string(), sanitize_function_schema(items));
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(composition) = sanitized_composition(object.get(key)) {
            sanitized.insert(key.to_string(), composition);
        }
    }
    if !sanitized.contains_key("type") && !has_composition(&sanitized) {
        sanitized.insert("type".to_string(), Value::String("object".to_string()));
    }
    Value::Object(sanitized)
}

fn schema_type<'a>(
    object: &'a serde_json::Map<String, Value>,
    nullable: &mut bool,
) -> Option<&'a str> {
    match object.get("type") {
        Some(Value::String(schema_type)) => supported_schema_type(schema_type),
        Some(Value::Array(types)) => {
            let mut selected = None;
            for schema_type in types.iter().filter_map(Value::as_str) {
                if schema_type == "null" {
                    *nullable = true;
                } else if selected.is_none() {
                    selected = supported_schema_type(schema_type);
                }
            }
            selected
        }
        _ if object.contains_key("properties") => Some("object"),
        _ if object.contains_key("items") => Some("array"),
        _ if object.contains_key("enum") || object.contains_key("const") => Some("string"),
        _ => None,
    }
}

fn supported_schema_type(schema_type: &str) -> Option<&'static str> {
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

fn sanitized_enum(value: Option<&Value>) -> Option<Vec<Value>> {
    let values = value?.as_array()?;
    let strings = values
        .iter()
        .filter_map(Value::as_str)
        .map(|value| Value::String(value.to_string()))
        .collect::<Vec<_>>();
    (!strings.is_empty()).then_some(strings)
}

fn sanitized_properties(value: Option<&Value>) -> Option<Value> {
    let properties = value?.as_object()?;
    let sanitized = properties
        .iter()
        .map(|(name, schema)| (name.clone(), sanitize_function_schema(schema)))
        .collect::<serde_json::Map<_, _>>();
    Some(Value::Object(sanitized))
}

fn sanitized_required(value: Option<&Value>) -> Option<Value> {
    let required = value?.as_array()?;
    let required = required
        .iter()
        .filter_map(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(|name| Value::String(name.to_string()))
        .collect::<Vec<_>>();
    (!required.is_empty()).then_some(Value::Array(required))
}
