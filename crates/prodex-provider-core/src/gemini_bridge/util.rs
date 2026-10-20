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
    prodex_mojo_core::json::gemini_tool_pattern_parts(value)
        .expect("compiled Mojo Gemini tool-pattern parser failed")
        .map(|(tool, pattern)| (tool.to_string(), pattern.to_string()))
}

pub fn gemini_provider_core_skip_context_path_name(name: &str) -> bool {
    prodex_mojo_core::json::gemini_skip_context_path_name(name)
        .expect("compiled Mojo Gemini context-name classifier failed")
}

pub fn gemini_provider_core_stream_error(value: &serde_json::Value) -> Option<(String, String)> {
    let mut document = crate::mojo_json::Document::default();
    // Acquire only the error member: successful SSE payloads may be large and
    // must not be recopied merely to ask whether an embedded error exists.
    document.member(value, "error");
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    let bytes = prodex_mojo_core::json::gemini_value_projection(
        &document.nodes,
        raw,
        prodex_mojo_core::json::GeminiValueOperation::StreamError,
    )
    .expect("compiled Mojo Gemini stream error projection returned invalid output");
    serde_json::from_slice(&bytes)
        .expect("compiled Mojo Gemini stream error projection must return an optional pair")
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
    fn compiled_gemini_stream_error_preserves_status_presence_and_i64_grammar() {
        let fallback = "provider_stream_error";
        let message_fallback = "Gemini stream returned an embedded provider error";
        for (value, expected) in [
            (serde_json::json!({}), None),
            (
                serde_json::json!({"error": null}),
                Some((fallback, message_fallback)),
            ),
            (
                serde_json::json!({"error": {}}),
                Some((fallback, message_fallback)),
            ),
            (
                serde_json::json!({"error": "unstructured"}),
                Some((fallback, message_fallback)),
            ),
            (
                serde_json::json!({"error":{"status":"E_CONN","message":"失败 🚀"}}),
                Some(("E_CONN", "失败 🚀")),
            ),
            (
                serde_json::json!({"error":{"code":"E_SECONDARY","message":""}}),
                Some(("E_SECONDARY", "")),
            ),
            (
                serde_json::json!({"error":{"status": null, "code": 404, "message": 9}}),
                Some((fallback, message_fallback)),
            ),
            (
                serde_json::json!({"error":{"status": false, "code": "OTHER"}}),
                Some((fallback, message_fallback)),
            ),
            (
                serde_json::json!({"error":{"status": 0}}),
                Some(("0", message_fallback)),
            ),
            (
                serde_json::json!({"error":{"status": -7}}),
                Some(("-7", message_fallback)),
            ),
            (
                serde_json::json!({"error":{"status": i64::MIN}}),
                Some(("-9223372036854775808", message_fallback)),
            ),
            (
                serde_json::json!({"error":{"status": i64::MAX}}),
                Some(("9223372036854775807", message_fallback)),
            ),
            (
                serde_json::json!({"error":{"status": u64::MAX}}),
                Some((fallback, message_fallback)),
            ),
            (
                serde_json::json!({"error":{"code": 1.25}}),
                Some((fallback, message_fallback)),
            ),
            (
                serde_json::json!({"error":{"code": 1.0}}),
                Some((fallback, message_fallback)),
            ),
            (
                serde_json::json!({"error":{"status": "", "message": ""}}),
                Some(("", "")),
            ),
            (
                serde_json::json!({"error":{"status": [], "message": []}}),
                Some((fallback, message_fallback)),
            ),
        ] {
            assert_eq!(
                gemini_provider_core_stream_error(&value),
                expected.map(|(code, message)| (code.to_owned(), message.to_owned())),
                "value={value:?}",
            );
        }
    }

    #[test]
    fn compiled_gemini_tool_pattern_parser_reaches_public_provider_boundary() {
        for (input, expected) in [
            ("cargo(rust tests)", Some(("cargo", "rust tests"))),
            (
                " \u{2003}検索 \u{2003}( \u{2003}foo.* \u{2003}) ",
                Some(("検索", "foo.*")),
            ),
            ("grep(abc) trailing )", Some(("grep", "abc) trailing"))),
            ("grep()", None),
            ("(regex)", None),
            ("x", None),
        ] {
            assert_eq!(
                gemini_provider_core_parse_command_specific_tool(input),
                expected.map(|(tool, pattern)| (tool.to_string(), pattern.to_string())),
                "{input:?}",
            );
        }
    }

    #[test]
    fn compiled_gemini_path_exclusion_reaches_public_provider_boundary() {
        for (name, excluded) in [
            (".git", true),
            ("node_modules", true),
            ("target", true),
            ("dist", true),
            ("build", true),
            ("__pycache__", true),
            ("vendor", true),
            ("node_modules-extra", false),
            ("vendor ", false),
            (".GIT", false),
            ("東京", false),
        ] {
            assert_eq!(gemini_provider_core_skip_context_path_name(name), excluded);
        }
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
