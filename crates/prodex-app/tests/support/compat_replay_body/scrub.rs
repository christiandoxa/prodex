fn compat_replay_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key == "authorization"
        || key == "authorization_token"
        || key == "api_key"
        || key == "x-api-key"
        || key == "cookie"
        || key.contains("token")
        || key.contains("secret")
        || key.contains("password")
        || key.contains("credential")
}

fn compat_replay_header_placeholder(header_name: &str) -> Option<&'static str> {
    let header_name = header_name.to_ascii_lowercase();
    if compat_replay_sensitive_key(&header_name) {
        return Some("<redacted>");
    }
    match header_name.as_str() {
        "chatgpt-account-id" => Some("<account_id>"),
        "session_id" | "x-session-id" | "x-claude-code-session-id" => Some("<session_id>"),
        "x-codex-turn-state" => Some("<turn_state>"),
        _ => None,
    }
}

fn compat_replay_scrub_string_array(value: &mut Value, placeholder: &str) {
    if let Value::Array(items) = value {
        for item in items {
            if !item.is_null() {
                *item = Value::String(placeholder.to_string());
            }
        }
    }
}

fn compat_replay_value_scrub(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for key in [
                "id",
                "request_id",
                "session_id",
                "response_id",
                "previous_response_id",
                "trace_id",
                "created_at",
                "updated_at",
                "started_at",
                "timestamp",
                "ts",
                "pid",
                "account_id",
                "call_id",
                "approval_request_id",
            ] {
                if let Some(entry) = map.get_mut(key)
                    && !entry.is_null()
                {
                    *entry = Value::String(format!("<{key}>"));
                }
            }
            if let Some(entry) = map.get_mut("response_ids") {
                compat_replay_scrub_string_array(entry, "<response_id>");
            }
            if let Some(entry) = map.get_mut("allowed_tools") {
                compat_replay_scrub_string_array(entry, "<tool_name>");
            }
            for (key, entry) in map.iter_mut() {
                if compat_replay_sensitive_key(key) && !entry.is_null() {
                    *entry = Value::String("<redacted>".to_string());
                }
            }
            if let Some(placeholder) = map
                .get("name")
                .and_then(Value::as_str)
                .and_then(compat_replay_header_placeholder)
                && let Some(entry) = map.get_mut("value")
                && !entry.is_null()
            {
                *entry = Value::String(placeholder.to_string());
            }

            for nested in map.values_mut() {
                compat_replay_value_scrub(nested);
            }
        }
        Value::Array(items) => {
            for nested in items {
                compat_replay_value_scrub(nested);
            }
        }
        _ => {}
    }
}

fn compat_replay_normalize_json(bytes: &[u8]) -> String {
    let mut value: Value =
        serde_json::from_slice(bytes).expect("replay payload should be valid JSON");
    compat_replay_value_scrub(&mut value);
    serde_json::to_string(&value).expect("replay payload should reserialize")
}

fn compat_replay_normalized_value(mut value: Value) -> Value {
    compat_replay_value_scrub(&mut value);
    value
}

fn compat_replay_assert_golden(actual: Value, expected_fixture: &str) {
    assert_eq!(
        compat_replay_normalized_value(actual),
        compat_replay_normalized_value(compat_replay_fixture_json(expected_fixture)),
        "compat replay fixture {expected_fixture} drifted"
    );
}
