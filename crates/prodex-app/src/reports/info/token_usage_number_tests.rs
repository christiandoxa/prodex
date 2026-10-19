use super::*;

#[test]
fn token_number_policy_preserves_text_and_json_event_values() {
    let expected = InfoTokenUsageEvent {
        timestamp: "fixture".to_string(),
        request: Some(0),
        profile: "pool beta".to_string(),
        transport: String::new(),
        source: "unknown".to_string(),
        input_tokens: u64::MAX,
        cached_input_tokens: 0,
        output_tokens: 15,
        reasoning_tokens: 0,
        generation_ms: Some(1000),
        output_tokens_per_second: Some(15.0),
    };
    let text = "[fixture] token_usage input_tokens=+18446744073709551615 cached_input_tokens=18446744073709551616 output_tokens=+15 reasoning_tokens=-1 request=+0 generation_ms=1000 output_tokens_per_second=.1 profile=\"pool beta\" transport=\"\"";
    let json = serde_json::json!({"timestamp": "fixture", "event": "token_usage", "fields": {
        "input_tokens": u64::MAX, "cached_input_tokens": "18446744073709551616", "output_tokens": "+15",
        "reasoning_tokens": false, "request": "+0", "generation_ms": 1000,
        "output_tokens_per_second": ".1", "profile": "pool beta", "transport": ""
    }}).to_string();
    for line in [text, &json] {
        assert_eq!(
            info_token_usage_event_from_line(line),
            Some(expected.clone())
        );
    }
}

#[test]
fn token_number_policy_distinguishes_completed_and_progress_requirements() {
    for line in [
        "token_usage",
        "token_usage input_tokens=bad",
        "token_usage input_tokens=1 output_tokens=bad",
    ] {
        assert_eq!(info_token_usage_event_from_line(line), None);
    }
    let expected = InfoTokenUsageEvent {
        profile: "unknown".into(),
        transport: "unknown".into(),
        source: "unknown".into(),
        input_tokens: 1,
        ..InfoTokenUsageEvent::default()
    };
    assert_eq!(
        info_token_usage_event_from_line("token_usage input_tokens=1"),
        Some(expected)
    );
    for input in ["", " input_tokens=bad"] {
        let line = format!("token_usage_progress{input} output_tokens=0 generation_ms=1");
        assert_eq!(
            info_token_usage_progress_event_from_line(&line),
            Some(InfoTokenUsageEvent {
                profile: "unknown".into(),
                transport: "unknown".into(),
                source: "unknown".into(),
                generation_ms: Some(1),
                ..InfoTokenUsageEvent::default()
            })
        );
    }
    for line in [
        "token_usage_progress input_tokens=1",
        "token_usage_progress output_tokens=bad",
    ] {
        assert_eq!(info_token_usage_progress_event_from_line(line), None);
    }
}

#[test]
fn token_number_policy_preserves_rate_rounding_at_the_event_boundary() {
    for (text, valid) in [
        ("5e-324", true),
        ("1e-324", false),
        ("1.7976931348623158e308", true),
        ("1.7976931348623159e308", false),
        ("1_0", false),
        (" 1", false),
    ] {
        let line = serde_json::json!({"event": "token_usage", "fields": {
            "input_tokens": 1, "output_tokens": 15, "generation_ms": 1000, "output_tokens_per_second": text,
        }}).to_string();
        let event = info_token_usage_event_from_line(&line).unwrap();
        assert_eq!(event.generation_ms, valid.then_some(1000));
        assert_eq!(event.output_tokens_per_second, valid.then_some(15.0));
    }
}
