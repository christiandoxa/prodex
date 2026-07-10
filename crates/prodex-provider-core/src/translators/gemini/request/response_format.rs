//! Gemini response_format to generationConfig mapping.

use serde_json::Value;

use super::schema::sanitize_schema;

pub(crate) fn gemini_apply_response_format(
    response_format: &Value,
    generation_config: &mut serde_json::Map<String, Value>,
) -> Result<(), String> {
    match response_format
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("text")
    {
        "text" => {}
        "json_object" => {
            generation_config.insert(
                "responseMimeType".to_string(),
                Value::String("application/json".to_string()),
            );
        }
        "json_schema" => {
            generation_config.insert(
                "responseMimeType".to_string(),
                Value::String("application/json".to_string()),
            );
            if let Some(schema) = response_format.get("schema") {
                generation_config.insert("responseJsonSchema".to_string(), sanitize_schema(schema));
            }
        }
        other => {
            return Err(format!(
                "Gemini response_format type `{other}` is not supported"
            ));
        }
    }
    Ok(())
}
