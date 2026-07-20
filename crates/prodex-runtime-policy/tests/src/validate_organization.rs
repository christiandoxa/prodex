use super::{Path, parse_policy, validate_runtime_policy_file};

#[test]
fn organization_selectors_are_strict_typed_and_bounded() {
    let valid = parse_policy(
        r#"
version = 1

[[governance.policy_rules]]
id = "deny.organization"
effect = "deny"
obligations = []
reason_code = "policy.organization_denied"

[governance.policy_rules.condition]
channel = "api"
group_id = "engineering"
department_id = "research"
"#,
    );
    validate_runtime_policy_file(&valid, Path::new("policy.toml"))
        .expect("organization selectors should validate");

    for field in ["group_id", "department_id"] {
        let mut invalid = valid.clone();
        let condition = &mut invalid.governance.policy_rules[0].condition;
        match field {
            "group_id" => condition.group_id = Some("x".repeat(129)),
            "department_id" => condition.department_id = Some("x".repeat(129)),
            _ => unreachable!(),
        }
        let error = validate_runtime_policy_file(&invalid, Path::new("policy.toml"))
            .expect_err("organization selectors must be bounded");
        assert!(
            error.to_string().contains("attribute selector"),
            "{field}: {error:#}"
        );
    }
}
