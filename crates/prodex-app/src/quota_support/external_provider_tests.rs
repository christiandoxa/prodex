use super::{
    ExternalQuotaDetail, ExternalQuotaInfo, anthropic_rate_limits_summary, parse_agy_quota_json,
};

fn ready_agy() -> ExternalQuotaInfo {
    ExternalQuotaInfo {
        provider: "Anti-Gravity".into(),
        account: None,
        plan: None,
        status: "Ready".into(),
        main: "quota information available".into(),
        reset: None,
        available: Some(true),
        details: vec![],
    }
}

#[test]
fn compiled_agy_projection_preserves_account_precedence_and_nested_details() {
    let value = serde_json::json!([
        {"account":"first", "credits":2},
        {"account":null, "email":"selected@example.com", "plan":"pro", "status":"",
         "credits":"not numeric", "usage":12.5, "count":9,
         "details":{"z":[false,null], "雪":"line\n\0🧭"},
         "models":["alpha", {"limit":3}], "nested":{"a":"b"}, "quotas":"ignored"},
        {"account":"selected@example.com", "credits":999}
    ]);
    let mut expected = ready_agy();
    expected.plan = Some("pro".into());
    expected.status = String::new();
    expected.main = "12.50 usage".into();
    expected.details = [
        ("count", "9"),
        ("details:z", "[false,null]"),
        ("details:雪", "line\n\0🧭"),
        ("models[0]", "alpha"),
        ("models[1]", "{\"limit\":3}"),
        ("nested", "{\"a\":\"b\"}"),
    ]
    .into_iter()
    .map(|(label, value)| ExternalQuotaDetail {
        label: label.into(),
        value: value.into(),
    })
    .collect();
    assert_eq!(
        parse_agy_quota_json(value.clone(), Some("selected@example.com")).unwrap(),
        expected
    );
    let mut first = ready_agy();
    first.account = Some("first".into());
    first.main = "2.00 credits available".into();
    assert_eq!(
        parse_agy_quota_json(value, Some("absent@example.com")).unwrap(),
        first
    );
    let mut email = ready_agy();
    email.account = Some("user@example.com".into());
    assert_eq!(
        parse_agy_quota_json(serde_json::json!({"email":"user@example.com"}), None).unwrap(),
        email
    );
}

#[test]
fn compiled_agy_projection_preserves_empty_arrays_and_non_object_account_shapes() {
    assert_eq!(
        parse_agy_quota_json(serde_json::json!([]), None)
            .unwrap_err()
            .to_string(),
        "no account found in agy output"
    );
    for value in [
        serde_json::Value::Null,
        serde_json::json!(false),
        serde_json::json!(2),
        serde_json::json!("text"),
        serde_json::json!([["nested array"]]),
        serde_json::json!({"models":true}),
    ] {
        assert_eq!(parse_agy_quota_json(value, None).unwrap(), ready_agy());
    }
    let mut expected = ready_agy();
    expected.account = Some(String::new());
    assert_eq!(
        parse_agy_quota_json(serde_json::json!([{}, {"account":""}]), Some("")).unwrap(),
        expected
    );
}

#[test]
fn compiled_agy_amount_plan_preserves_original_float_bits_and_fixed_precision() {
    let mut numbers = vec![
        serde_json::Number::from(u64::MAX),
        serde_json::Number::from(i64::MIN),
        serde_json::Number::from(9_007_199_254_740_993_u64),
    ];
    for value in [
        -0.0,
        0.125,
        2.675,
        -2.675,
        f64::MIN_POSITIVE,
        f64::EPSILON,
        1e308,
        -1e308,
    ] {
        numbers.push(serde_json::Number::from_f64(value).unwrap());
    }
    let mut bits = 0x1234_5678_9abc_def0_u64;
    for _ in 0..64 {
        bits = bits.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        if let Some(number) = serde_json::Number::from_f64(f64::from_bits(bits)) {
            numbers.push(number);
        }
    }
    for number in numbers {
        let expected = format!("{:.2} credits available", number.as_f64().unwrap());
        let value =
            serde_json::json!([{"credits":0}, {"account":"target", "credits":number, "usage":1}]);
        assert_eq!(
            parse_agy_quota_json(value, Some("target")).unwrap().main,
            expected
        );
    }
}

#[test]
fn compiled_anthropic_projection_preserves_counts_first_three_and_scalar_details() {
    let value = serde_json::json!({"data":[
        {"group_type":"organization"},
        {"group_type":"model_group", "models":[null,"ignored"], "limits":[]},
        {"limits":[{}, {"type":1,"value":2}, {"type":"rpm","value":42},
            {"type":"null","value":null}, {"type":"note","value":"line\n\0🧭"},
            {"type":"nested","value":{"a":true}}]},
        {"group_type":"model_group", "models":["fourth"], "limits":[]}
    ]});
    let expected = [
        ("Rate groups", "4"),
        ("Model groups", "2"),
        ("model_group", ""),
        (
            "rate_limit",
            "rpm=42, null=null, note=line\n\0🧭, nested={\"a\":true}",
        ),
    ]
    .into_iter()
    .map(|(label, value)| ExternalQuotaDetail {
        label: label.into(),
        value: value.into(),
    })
    .collect::<Vec<_>>();
    assert_eq!(
        anthropic_rate_limits_summary(&value).unwrap(),
        ("2 model rate group(s)".into(), expected)
    );
    let expected = ["Rate groups", "Model groups"]
        .map(|label| ExternalQuotaDetail {
            label: label.into(),
            value: "0".into(),
        })
        .to_vec();
    for value in [
        serde_json::Value::Null,
        serde_json::json!({}),
        serde_json::json!({"data":null}),
        serde_json::json!({"data":[]}),
    ] {
        assert_eq!(
            anthropic_rate_limits_summary(&value).unwrap(),
            ("0 rate group(s)".into(), expected.clone())
        );
    }
    let summary = anthropic_rate_limits_summary(&serde_json::json!({"data":[
        {"group_type":"model_group", "models":["東京🧭"], "limits":[{"type":"tokens","value":[1,2]}]}
    ]})).unwrap();
    assert_eq!(
        summary.1[2],
        ExternalQuotaDetail {
            label: "東京🧭".into(),
            value: "tokens=[1,2]".into()
        }
    );
}
