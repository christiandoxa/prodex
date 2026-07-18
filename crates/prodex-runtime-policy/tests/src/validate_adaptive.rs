use super::{RuntimePolicyFile, validate_runtime_policy_file};
use std::path::Path;

#[test]
fn accepts_live_adaptive_routing() {
    let policy: RuntimePolicyFile = toml::from_str(
        r#"
version = 1

[gateway.adaptive_routing]
enabled = true
shadow_mode = true
window_size = 64
min_samples = 8

[[gateway.route_aliases]]
alias = "adaptive"
models = ["model-a", "model-b"]
"#,
    )
    .unwrap();

    validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect("live adaptive routing must validate");
}

#[test]
fn rejects_adaptive_minimum_larger_than_window() {
    let policy: RuntimePolicyFile = toml::from_str(
        r#"
version = 1

[gateway.adaptive_routing]
enabled = true
window_size = 4
min_samples = 5

[[gateway.route_aliases]]
alias = "adaptive"
models = ["model-a", "model-b"]
"#,
    )
    .unwrap();

    let error = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("an unreachable sample threshold must fail");
    assert!(error.to_string().contains("must not exceed"));
}
