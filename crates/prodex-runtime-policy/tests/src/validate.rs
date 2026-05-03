use super::{RuntimePolicyFile, validate_runtime_policy_file};
use std::path::Path;

fn parse_policy(input: &str) -> RuntimePolicyFile {
    toml::from_str(input).expect("policy TOML should parse")
}

#[test]
fn validate_runtime_policy_allows_zero_websocket_executor_overflow_capacities() {
    let policy = parse_policy(
        r#"
version = 1

[runtime_proxy]
websocket_connect_worker_count = 4
websocket_connect_queue_capacity = 16
websocket_connect_overflow_capacity = 0
websocket_dns_worker_count = 2
websocket_dns_queue_capacity = 8
websocket_dns_overflow_capacity = 0
"#,
    );

    validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect("zero websocket overflow capacities should be valid");
}

#[test]
fn validate_runtime_policy_rejects_zero_websocket_executor_non_overflow_values() {
    let policy = parse_policy(
        r#"
version = 1

[runtime_proxy]
websocket_connect_worker_count = 0
websocket_connect_overflow_capacity = 0
websocket_dns_overflow_capacity = 0
"#,
    );

    let err = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("zero websocket executor worker count should be rejected");
    assert!(
        err.to_string()
            .contains("runtime_proxy.websocket_connect_worker_count")
    );
}
