use super::{RuntimePolicyFile, validate_runtime_policy_file};
use std::path::Path;

#[test]
fn rejects_inactive_adaptive_routing() {
    let policy: RuntimePolicyFile = toml::from_str(
        r#"
version = 1

[gateway.adaptive_routing]
enabled = true
shadow_mode = true
window_size = 64
min_samples = 8
"#,
    )
    .unwrap();

    let error = validate_runtime_policy_file(&policy, Path::new("policy.toml"))
        .expect_err("inactive adaptive routing must fail instead of silently doing nothing");
    assert!(error.to_string().contains("reserved and unsupported"));
}
