use super::project_external_quota;
use crate::quota_support::{ExternalQuotaDetail, ExternalQuotaInfo};
use prodex_mojo_core::json::ExternalQuotaProjection;

#[test]
fn compiled_deepseek_quota_preserves_ordered_balances_and_string_defaults() {
    let value = serde_json::json!({"is_available":true, "balance_infos":[
        {"currency":"USD", "total_balance":"12.500", "granted_balance":"2.5", "topped_up_balance":"10.000"},
        null,
        {"currency":"雪\0🧭", "total_balance":7, "granted_balance":false, "topped_up_balance":"quote\""},
        {"currency":"", "total_balance":"", "granted_balance":"", "topped_up_balance":""}
    ]});
    let expected = ExternalQuotaInfo {
        provider: "DeepSeek".into(),
        account: None,
        plan: Some("api-key".into()),
        status: "Ready".into(),
        main: "USD 12.500 | balance - | 雪\0🧭 - | \u{20}".into(),
        reset: None,
        available: Some(true),
        details: [
            ("USD balance", "total 12.500; granted 2.5; topped up 10.000"),
            ("balance balance", "total -; granted -; topped up -"),
            ("雪\0🧭 balance", "total -; granted -; topped up quote\""),
            (" balance", "total ; granted ; topped up "),
        ]
        .into_iter()
        .map(|(label, value)| ExternalQuotaDetail {
            label: label.into(),
            value: value.into(),
        })
        .collect(),
    };
    assert_eq!(
        project_external_quota(&value, ExternalQuotaProjection::DeepSeekBalance).unwrap(),
        Some(expected)
    );
    let blocked = ExternalQuotaInfo {
        provider: "DeepSeek".into(),
        account: None,
        plan: Some("api-key".into()),
        status: "Blocked".into(),
        main: "balance unavailable".into(),
        reset: None,
        available: Some(false),
        details: vec![],
    };
    for value in [
        serde_json::Value::Null,
        serde_json::json!({}),
        serde_json::json!({"balance_infos":{}}),
        serde_json::json!({"is_available":1}),
        serde_json::json!({"is_available":"true"}),
        serde_json::json!({"is_available":false,"balance_infos":[]}),
    ] {
        assert_eq!(
            project_external_quota(&value, ExternalQuotaProjection::DeepSeekBalance).unwrap(),
            Some(blocked.clone())
        );
    }
}

#[test]
fn compiled_local_models_quota_preserves_count_presence_and_host_metadata() {
    for (value, main) in [
        (serde_json::json!({"data":[]}), "0 model(s)"),
        (
            serde_json::json!({"data":[null,{},"same","same"]}),
            "4 model(s)",
        ),
        (serde_json::Value::Null, "models endpoint reachable"),
        (serde_json::json!({"data":{}}), "models endpoint reachable"),
    ] {
        let input = serde_json::json!({"value":value,"account":"http://example.com/","models_url":"http://example.com/v1/models"});
        let expected = ExternalQuotaInfo {
            provider: "Local OpenAI-compatible".into(),
            account: Some("http://example.com/".into()),
            plan: Some("local".into()),
            status: "Reachable".into(),
            main: main.into(),
            reset: None,
            available: Some(true),
            details: vec![ExternalQuotaDetail {
                label: "Models URL".into(),
                value: "http://example.com/v1/models".into(),
            }],
        };
        assert_eq!(
            project_external_quota(&input, ExternalQuotaProjection::LocalModels).unwrap(),
            Some(expected)
        );
    }
    for input in [
        serde_json::json!({"value":{},"account":null,"models_url":"http://example.com/models"}),
        serde_json::json!({"value":{},"account":"http://example.com/"}),
    ] {
        assert!(
            project_external_quota::<ExternalQuotaInfo>(
                &input,
                ExternalQuotaProjection::LocalModels
            )
            .is_err()
        );
    }
}

#[test]
fn compiled_virtual_quota_projections_are_isolated_between_threads() {
    std::thread::scope(|scope| {
        for worker in 0..4 {
            scope.spawn(move || {
                for index in 0..16 {
                    let currency=format!("C{worker}");
                    let value=serde_json::json!({"is_available":true,"balance_infos":[{"currency":currency,"total_balance":index.to_string()}]});
                    let info=project_external_quota::<ExternalQuotaInfo>(&value,ExternalQuotaProjection::DeepSeekBalance).unwrap().unwrap();
                    assert_eq!(info.main,format!("C{worker} {index}"));
                    assert_eq!(info.details[0].label,format!("C{worker} balance"));
                }
            });
        }
    });
}
