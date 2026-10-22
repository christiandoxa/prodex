use super::provider_core_chat_compatible_validate_top_level_request_shape as validate;
use serde_json::json;

#[test]
fn chat_request_shape_preserves_optional_fields_and_unicode() {
    for request in [
        json!({}),
        json!({"stream":false,"instructions":"","previous_response_id":""}),
        json!({"model":" 東京🧭 ","stream":true,"instructions":"i","previous_response_id":"r"}),
        json!({"model":"\u{200b}"}),
        json!({"model":"\u{feff}"}),
        json!({"model":" \0 "}),
        json!({"input":[{"model":null}],"unknown":{"stream":null}}),
        json!({"model":"x".repeat(131_072)}),
    ] {
        assert_eq!(validate(&request, "Gemini"), Ok(()));
    }
}

#[test]
fn chat_request_shape_preserves_exact_errors_and_precedence() {
    let whitespace = "\t\n\u{b}\u{c}\r \u{85}\u{a0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}";
    for (request, suffix) in [
        (json!(null), " request body must be a JSON object"),
        (json!(false), " request body must be a JSON object"),
        (json!(42), " request body must be a JSON object"),
        (json!("text"), " request body must be a JSON object"),
        (json!([]), " request body must be a JSON object"),
        (
            json!({"model":null,"stream":null,"instructions":null,"previous_response_id":null}),
            " model must be a string",
        ),
        (json!({"model":[],"stream":null}), " model must be a string"),
        (
            json!({"model":"","stream":null}),
            " model must not be empty",
        ),
        (
            json!({"model":whitespace,"instructions":null}),
            " model must not be empty",
        ),
        (
            json!({"stream":null,"instructions":null,"previous_response_id":null}),
            " stream must be a boolean",
        ),
        (json!({"stream":"false"}), " stream must be a boolean"),
        (
            json!({"instructions":7,"previous_response_id":null}),
            " instructions must be a string",
        ),
        (
            json!({"instructions":null}),
            " instructions must be a string",
        ),
        (
            json!({"previous_response_id":null}),
            " previous_response_id must be a string",
        ),
        (
            json!({"previous_response_id":false}),
            " previous_response_id must be a string",
        ),
    ] {
        for provider in ["DeepSeek", "Gemini", "Kiro", "", " 東京\n\0 "] {
            assert_eq!(
                validate(&request, provider),
                Err(format!("{provider}{suffix}"))
            );
        }
    }
}
