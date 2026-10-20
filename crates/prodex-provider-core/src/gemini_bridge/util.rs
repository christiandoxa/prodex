//! Small Gemini bridge value parsing and collection helpers.

use prodex_mojo_core::provider_constraints::provider_boolean_token;

fn gemini_collect_values(
    value: Option<&serde_json::Value>,
    operation: prodex_mojo_core::json::GeminiValueOperation,
) -> Vec<String> {
    let envelope = serde_json::json!({"value": value});
    let mut document = crate::mojo_json::Document::default();
    document.root(&envelope);
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    let bytes = prodex_mojo_core::json::gemini_value_projection(&document.nodes, raw, operation)
        .expect("compiled Mojo Gemini value collection returned invalid output");
    serde_json::from_slice(&bytes)
        .expect("compiled Mojo Gemini value collection must emit string array")
}

pub fn gemini_provider_core_collect_string_values(
    value: Option<&serde_json::Value>,
    values: &mut Vec<String>,
) {
    values.extend(gemini_collect_values(
        value,
        prodex_mojo_core::json::GeminiValueOperation::Strings,
    ));
}

pub fn gemini_provider_core_collect_path_values(
    value: Option<&serde_json::Value>,
    paths: &mut Vec<std::path::PathBuf>,
) {
    paths.extend(
        gemini_collect_values(value, prodex_mojo_core::json::GeminiValueOperation::Paths)
            .into_iter()
            .map(std::path::PathBuf::from),
    );
}

pub fn gemini_provider_core_collect_input_texts(
    value: Option<&serde_json::Value>,
    texts: &mut Vec<String>,
) {
    texts.extend(gemini_collect_values(
        value,
        prodex_mojo_core::json::GeminiValueOperation::InputTexts,
    ));
}

pub fn gemini_provider_core_bool_value(value: &serde_json::Value) -> Option<bool> {
    match value {
        serde_json::Value::Bool(value) => Some(*value),
        serde_json::Value::Number(number) => Some(number.as_i64().unwrap_or_default() != 0),
        serde_json::Value::String(value) => gemini_provider_core_bool_str(value),
        _ => None,
    }
}

pub fn gemini_provider_core_bool_str(value: &str) -> Option<bool> {
    provider_boolean_token(value).expect("Mojo Gemini boolean token classification failed")
}

pub fn gemini_provider_core_parse_command_specific_tool(value: &str) -> Option<(String, String)> {
    let value = value.trim();
    let open = value.find('(')?;
    let close = value.rfind(')')?;
    if close <= open {
        return None;
    }
    let tool = value[..open].trim();
    let pattern = value[open + 1..close].trim();
    if tool.is_empty() || pattern.is_empty() {
        return None;
    }
    Some((tool.to_string(), pattern.to_string()))
}

pub fn gemini_provider_core_skip_context_path_name(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | "build" | "__pycache__" | "vendor"
    )
}

pub fn gemini_provider_core_stream_error(value: &serde_json::Value) -> Option<(String, String)> {
    let error = value.get("error")?;
    let code = error
        .get("status")
        .or_else(|| error.get("code"))
        .and_then(|code| {
            code.as_str()
                .map(str::to_string)
                .or_else(|| code.as_i64().map(|code| code.to_string()))
        })
        .unwrap_or_else(|| "provider_stream_error".to_string());
    let message = error
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Gemini stream returned an embedded provider error")
        .to_string();
    Some((code, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_gemini_collect_string_values_preserves_nested_array_order_and_existing_prefix() {
        let input = serde_json::json!([
            "", ["first", [null, false, "東京", {"content": "opaque"}]],
            42, "last", [],
        ]);
        let mut values = vec!["existing".to_string()];
        gemini_provider_core_collect_string_values(Some(&input), &mut values);
        gemini_provider_core_collect_string_values(None, &mut values);
        assert_eq!(values, ["existing", "", "first", "東京", "last"]);
        gemini_provider_core_collect_string_values(Some(&serde_json::json!("!")), &mut values);
        assert_eq!(values.last().map(String::as_str), Some("!"));
    }

    #[test]
    fn compiled_gemini_collect_path_values_trims_unicode_and_preserves_array_order() {
        use std::path::PathBuf;
        let input = serde_json::json!([
            "  /tmp/a  ", "", " \u{2003} ",
            [" \u{2003}/tmp/東京\u{2003}", 4, null, "  relative/path  "],
            {"content":"/tmp/opaque"},
        ]);
        let mut paths = vec![PathBuf::from("seed")];
        gemini_provider_core_collect_path_values(Some(&input), &mut paths);
        gemini_provider_core_collect_path_values(Some(&serde_json::Value::Null), &mut paths);
        assert_eq!(
            paths,
            [
                PathBuf::from("seed"),
                PathBuf::from("/tmp/a"),
                PathBuf::from("/tmp/東京"),
                PathBuf::from("relative/path"),
            ]
        );
    }

    #[test]
    fn compiled_gemini_collect_input_texts_keeps_object_field_priority_and_duplicate_content() {
        let input = serde_json::json!([
            "top",
            {"text":"header", "content":"body", "ignored":"ignore"},
            {"text": 22, "content":"child"},
            {"text":null, "content":["nested", {"text":"inner", "content":{"content":"leaf"}}]},
            {"text":"", "content":""},
            {"content":true},
            {"content":{"text":"branch","content":"tail"}},
            ["end", {"text":"standalone","other":["excluded"]}],
        ]);
        let mut texts = vec!["seed".to_string()];
        gemini_provider_core_collect_input_texts(Some(&input), &mut texts);
        assert_eq!(
            texts,
            [
                "seed",
                "top",
                "header",
                "body",
                "child",
                "nested",
                "inner",
                "leaf",
                "leaf",
                "",
                "",
                "branch",
                "tail",
                "end",
                "standalone",
            ]
        );
        gemini_provider_core_collect_input_texts(None, &mut texts);
        assert_eq!(texts.len(), 15);
    }

    #[test]
    fn gemini_bool_tokens_use_mojo_policy() {
        assert_eq!(gemini_provider_core_bool_str(" YES "), Some(true));
        assert_eq!(
            gemini_provider_core_bool_str("\u{2003}off\u{2003}"),
            Some(false)
        );
        assert_eq!(gemini_provider_core_bool_str("maybe"), None);
    }
}
