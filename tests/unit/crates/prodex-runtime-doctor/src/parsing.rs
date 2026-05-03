use super::*;

#[test]
fn runtime_doctor_parse_message_fields_handles_quoted_structured_values() {
    let fields = runtime_doctor_parse_message_fields(
        "stream_read_error request=7 transport=http error=\"failed with spaces\" empty=\"\"",
    );

    assert_eq!(fields.get("request").map(String::as_str), Some("7"));
    assert_eq!(
        fields.get("error").map(String::as_str),
        Some("failed with spaces")
    );
    assert_eq!(fields.get("empty").map(String::as_str), Some(""));
}
