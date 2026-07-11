use super::*;

#[test]
fn runtime_proxy_anthropic_reasoning_effort_normalizes_output_config_levels() {
    let _effort_guard = TestEnvVarGuard::unset("PRODEX_CLAUDE_REASONING_EFFORT");
    let cases = [
        ("gpt-5.4", "low", Some("low")),
        ("gpt-5.4", "medium", Some("medium")),
        ("gpt-5.4", "high", Some("high")),
        ("gpt-5.4", "max", Some("xhigh")),
        ("gpt-5", "max", Some("high")),
        ("gpt-5.4", "HIGH", Some("high")),
        ("gpt-5.4", "unknown", None),
    ];

    for (target_model, input_effort, expected) in cases {
        let value = serde_json::json!({
            "output_config": {
                "effort": input_effort,
            }
        });
        assert_eq!(
            runtime_proxy_anthropic_reasoning_effort(&value, target_model).as_deref(),
            expected,
            "input effort {input_effort:?} normalized incorrectly for target model {target_model}"
        );
    }
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_max_effort_to_xhigh_for_supported_model() {
    let _effort_guard = TestEnvVarGuard::unset("PRODEX_CLAUDE_REASONING_EFFORT");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5.4",
            "thinking": {
                "type": "adaptive"
            },
            "output_config": {
                "effort": "max",
            },
            "messages": [
                {
                    "role": "user",
                    "content": "hello"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("xhigh")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_keeps_max_effort_at_high_for_legacy_model() {
    let _effort_guard = TestEnvVarGuard::unset("PRODEX_CLAUDE_REASONING_EFFORT");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5",
            "thinking": {
                "type": "adaptive"
            },
            "output_config": {
                "effort": "max",
            },
            "messages": [
                {
                    "role": "user",
                    "content": "hello"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("high")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_honors_reasoning_override_env() {
    let _effort_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_REASONING_EFFORT", "xhigh");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5.2",
            "thinking": {
                "type": "adaptive"
            },
            "output_config": {
                "effort": "low",
            },
            "messages": [
                {
                    "role": "user",
                    "content": "hello"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("xhigh")
    );
}

