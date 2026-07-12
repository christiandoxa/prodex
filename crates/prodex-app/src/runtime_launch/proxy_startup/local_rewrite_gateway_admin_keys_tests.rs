use super::*;

#[test]
fn gateway_key_generation_error_uses_stable_redacted_message() {
    let error = runtime_gateway_admin_key_generation_failed_error();
    assert_eq!(error.test_status(), 500);
    assert_eq!(error.test_code(), "gateway_key_generation_failed");
    assert_eq!(
        error.test_message(),
        "gateway key token could not be generated"
    );
}

#[test]
fn supplied_token_is_trimmed_taken_and_not_left_in_json_projection() {
    let raw = " raw-token-debug-secret ";
    let mut body = serde_json::json!({"token": raw, "team_id": "team-a"});

    let token = take_supplied_token(&mut body).unwrap();

    assert_eq!(token.as_str(), "raw-token-debug-secret");
    assert_eq!(body["token"], "");
    assert!(!format!("{body:?}").contains("raw-token-debug-secret"));
}

#[test]
fn empty_or_non_string_supplied_tokens_are_absent() {
    for value in [
        serde_json::json!("   "),
        serde_json::json!(null),
        serde_json::json!(42),
    ] {
        let mut body = serde_json::json!({"token": value});
        assert!(take_supplied_token(&mut body).is_none());
    }
}

#[test]
fn mutation_handlers_use_application_planner_and_atomic_store_only() {
    let source = include_str!("local_rewrite_gateway_admin_keys.rs");
    for required in [
        "runtime_gateway_admin_mutation_execution",
        "plan_application_gateway_virtual_key_mutation",
        "runtime_gateway_mutate_admin_key_store_atomic",
        "runtime_gateway_apply_virtual_key_projection",
        "Zeroizing",
    ] {
        assert!(source.contains(required), "missing {required}");
    }
    for removed in [
        "runtime_gateway_mutate_admin_key_store(shared",
        "runtime_gateway_apply_virtual_key_patch",
        "runtime_gateway_admin_virtual_key_lifecycle_response",
        "plan_application_virtual_key_lifecycle",
        "runtime_gateway_audit_admin_key_event",
        "sha256:gateway-virtual-key-create",
        "sha256:gateway-virtual-key-rotate",
    ] {
        assert!(!source.contains(removed), "legacy path remains: {removed}");
    }
}

#[test]
fn token_generation_occurs_inside_atomic_executor_after_idempotency_check() {
    let source = include_str!("local_rewrite_gateway_admin_keys.rs");
    let create = source
        .split("pub(super) fn runtime_gateway_admin_create_key_response")
        .nth(1)
        .unwrap()
        .split("pub(super) fn runtime_gateway_admin_update_key_response")
        .next()
        .unwrap();
    assert!(
        create
            .find("runtime_gateway_mutate_admin_key_store_atomic")
            .unwrap()
            < create
                .find("runtime_gateway_generate_virtual_key_token")
                .unwrap()
    );
}

#[test]
fn policy_source_preflight_is_narrow_and_stable() {
    let source = include_str!("local_rewrite_gateway_admin_keys.rs");
    assert!(source.contains("Policy keys live outside the mutable admin store"));
    assert!(source.contains("RuntimeGatewayVirtualKeySource::Policy"));
    assert!(source.contains("gateway_key_read_only"));
}

#[test]
fn if_match_is_checked_against_authoritative_record_epoch() {
    let current = RuntimeGatewayStoredVirtualKey {
        name: "key-a".to_string(),
        token_hash_base64: "hash".to_string(),
        virtual_key_id: Some(prodex_domain::VirtualKeyId::new().to_string()),
        tenant_id: None,
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        allowed_models: Vec::new(),
        budget_microusd: None,
        request_budget: None,
        rpm_limit: None,
        tpm_limit: None,
        disabled: Some(false),
        created_at_epoch: 1,
        updated_at_epoch: 7,
    };
    let exact = prodex_domain::EntityTag::new("\"gateway-key-7\"").unwrap();
    let wildcard = prodex_domain::EntityTag::new("*").unwrap();
    let stale = prodex_domain::EntityTag::new("\"gateway-key-6\"").unwrap();

    assert!(enforce_key_entity_tag(None, &current).is_ok());
    assert!(enforce_key_entity_tag(Some(&exact), &current).is_ok());
    assert!(enforce_key_entity_tag(Some(&wildcard), &current).is_ok());
    let error = enforce_key_entity_tag(Some(&stale), &current).unwrap_err();
    assert_eq!(error.test_status(), 412);
    assert_eq!(error.test_code(), "precondition_failed");
}

#[test]
fn etag_remains_stable() {
    assert_eq!(runtime_gateway_admin_key_etag(Some(7)), "\"gateway-key-7\"");
    assert_eq!(runtime_gateway_admin_key_etag(None), "\"gateway-key-0\"");
}
