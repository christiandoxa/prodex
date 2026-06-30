use super::*;

#[test]
fn app_server_broker_parses_jsonrpc_lifecycle_methods() {
    for (method, expected) in [
        ("initialize", AppServerBrokerMethod::Initialize),
        ("notifications/initialized", AppServerBrokerMethod::Initialized),
        ("thread/start", AppServerBrokerMethod::ThreadStart),
        ("thread/resume", AppServerBrokerMethod::ThreadResume),
        ("thread/fork", AppServerBrokerMethod::ThreadFork),
        ("turn/start", AppServerBrokerMethod::TurnStart),
        ("turn/cancel", AppServerBrokerMethod::TurnCancel),
        ("unknown/method", AppServerBrokerMethod::Other),
    ] {
        let request = parse_app_server_broker_request(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": method,
            "params": {}
        }))
        .expect("valid JSON-RPC request should parse");

        assert_eq!(request.method, expected);
        assert_eq!(request.method.label(), expected.label());
    }
}

#[test]
fn app_server_broker_rejects_non_jsonrpc_payloads_and_reports_disabled_contract() {
    assert!(parse_app_server_broker_request(&serde_json::json!({"method":"initialize"})).is_none());

    let contract = app_server_broker_contract_json();
    assert_eq!(contract["enabled_by_default"], serde_json::Value::Bool(false));
    assert_eq!(contract["default_mode"], "direct-passthrough");
    assert!(
        contract["lifecycle_methods"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String("turn/start".to_string()))
    );
    assert_eq!(
        contract["affinity"]["continuation_affinity_wins"],
        serde_json::Value::Bool(true)
    );
}
