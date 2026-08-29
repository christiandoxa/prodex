use super::*;

fn stored_scim_user_for_tests() -> RuntimeGatewayScimUser {
    RuntimeGatewayScimUser {
        id: "user-1".to_string(),
        user_name: "alice@example.com".to_string(),
        external_id: None,
        display_name: None,
        active: true,
        role: Some("admin".to_string()),
        tenant_id: Some("tenant-a".to_string()),
        team_id: None,
        project_id: None,
        user_id: Some("user-1".to_string()),
        group_ids: vec!["engineering".to_string()],
        department_id: Some("research".to_string()),
        budget_id: None,
        allowed_key_prefixes: vec!["tenant-a-".to_string()],
        created_at_epoch: 1,
        updated_at_epoch: 2,
    }
}

fn policy_entry_for_scim_tests() -> RuntimeGatewayVirtualKeyEntry {
    RuntimeGatewayVirtualKeyEntry {
        virtual_key_id: None,
        key: runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: "policy-key".to_string(),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: Some("user-1".to_string()),
            budget_id: None,
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("secret"),
            allowed_models: Vec::new(),
            budget_microusd: None,
            request_budget: None,
            rpm_limit: None,
            tpm_limit: None,
        },
        source: RuntimeGatewayVirtualKeySource::Policy,
        tenant_id: Some("tenant-a".to_string()),
        group_ids: Vec::new(),
        department_id: None,
        created_at_epoch: None,
        updated_at_epoch: None,
        disabled: false,
    }
}

#[test]
fn scim_policy_attributes_require_one_active_same_tenant_identity() {
    let user = stored_scim_user_for_tests();
    let mut entries = vec![policy_entry_for_scim_tests()];
    runtime_gateway_apply_scim_policy_attributes(&mut entries, std::slice::from_ref(&user));
    assert_eq!(entries[0].group_ids, ["engineering"]);
    assert_eq!(entries[0].department_id.as_deref(), Some("research"));

    let mut cross_tenant = user.clone();
    cross_tenant.tenant_id = Some("tenant-b".to_string());
    runtime_gateway_apply_scim_policy_attributes(&mut entries, std::slice::from_ref(&cross_tenant));
    assert!(entries[0].group_ids.is_empty());
    assert_eq!(entries[0].department_id, None);

    runtime_gateway_apply_scim_policy_attributes(&mut entries, &[user.clone(), user]);
    assert!(entries[0].group_ids.is_empty());
    assert_eq!(entries[0].department_id, None);
}

#[test]
fn policy_virtual_key_effective_id_is_stable_tenant_scoped_uuid_v8() {
    let tenant_id = prodex_domain::TenantId::new();
    let entry = RuntimeGatewayVirtualKeyEntry {
        virtual_key_id: None,
        key: runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: "Policy-Key".to_string(),
            tenant_id: Some(tenant_id.to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("secret"),
            allowed_models: Vec::new(),
            budget_microusd: None,
            request_budget: None,
            rpm_limit: None,
            tpm_limit: None,
        },
        source: RuntimeGatewayVirtualKeySource::Policy,
        tenant_id: Some(tenant_id.to_string()),
        group_ids: Vec::new(),
        department_id: None,
        created_at_epoch: None,
        updated_at_epoch: None,
        disabled: false,
    };
    let first = runtime_gateway_virtual_key_effective_id(&entry).unwrap();
    let mut same = entry.clone();
    same.key.name = "policy-key".to_string();
    let second = runtime_gateway_virtual_key_effective_id(&same).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.as_uuid().get_version_num(), 8);

    same.tenant_id = Some(prodex_domain::TenantId::new().to_string());
    assert_ne!(
        first,
        runtime_gateway_virtual_key_effective_id(&same).unwrap()
    );
}

#[test]
fn stored_key_converts_to_admin_entry_with_exact_name() {
    let token_hash =
        runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("secret").hash_base64();
    let record = RuntimeGatewayStoredVirtualKey {
        name: "alpha".to_string(),
        token_hash_base64: token_hash,
        virtual_key_id: Some(prodex_domain::VirtualKeyId::new().to_string()),
        tenant_id: Some("tenant-a".to_string()),
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        allowed_models: vec!["gpt-5".to_string()],
        budget_microusd: Some(1_000),
        request_budget: Some(10),
        rpm_limit: Some(5),
        tpm_limit: Some(500),
        disabled: Some(true),
        created_at_epoch: 1,
        updated_at_epoch: 2,
    };

    let entry = runtime_gateway_virtual_key_entry_from_stored(&record).unwrap();

    assert_eq!(entry.key.name, "alpha");
    assert!(entry.virtual_key_id.is_some());
    assert_eq!(entry.source, RuntimeGatewayVirtualKeySource::Admin);
    assert!(entry.disabled);
    assert_eq!(entry.created_at_epoch, Some(1));
    assert_eq!(entry.updated_at_epoch, Some(2));
}

#[test]
fn stored_key_rejects_padded_name() {
    let token_hash =
        runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("secret").hash_base64();
    let record = RuntimeGatewayStoredVirtualKey {
        name: " alpha ".to_string(),
        token_hash_base64: token_hash,
        virtual_key_id: Some(prodex_domain::VirtualKeyId::new().to_string()),
        tenant_id: Some("tenant-a".to_string()),
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        allowed_models: vec!["gpt-5".to_string()],
        budget_microusd: Some(1_000),
        request_budget: Some(10),
        rpm_limit: Some(5),
        tpm_limit: Some(500),
        disabled: Some(false),
        created_at_epoch: 1,
        updated_at_epoch: 2,
    };

    assert!(runtime_gateway_virtual_key_entry_from_stored(&record).is_none());
}

#[test]
fn stored_key_rejects_padded_token_hash() {
    let token_hash =
        runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("secret").hash_base64();
    let record = RuntimeGatewayStoredVirtualKey {
        name: "alpha".to_string(),
        token_hash_base64: format!(" {token_hash} "),
        virtual_key_id: Some(prodex_domain::VirtualKeyId::new().to_string()),
        tenant_id: Some("tenant-a".to_string()),
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        allowed_models: vec!["gpt-5".to_string()],
        budget_microusd: Some(1_000),
        request_budget: Some(10),
        rpm_limit: Some(5),
        tpm_limit: Some(500),
        disabled: Some(false),
        created_at_epoch: 1,
        updated_at_epoch: 2,
    };

    assert!(runtime_gateway_virtual_key_entry_from_stored(&record).is_none());
}

#[test]
fn stored_key_rejects_padded_governance_scope() {
    let token_hash =
        runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("secret").hash_base64();
    let record = RuntimeGatewayStoredVirtualKey {
        name: "alpha".to_string(),
        token_hash_base64: token_hash,
        virtual_key_id: Some(prodex_domain::VirtualKeyId::new().to_string()),
        tenant_id: Some(" tenant-a ".to_string()),
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        allowed_models: vec!["gpt-5".to_string()],
        budget_microusd: Some(1_000),
        request_budget: Some(10),
        rpm_limit: Some(5),
        tpm_limit: Some(500),
        disabled: Some(false),
        created_at_epoch: 1,
        updated_at_epoch: 2,
    };

    assert!(runtime_gateway_virtual_key_entry_from_stored(&record).is_none());
}

#[test]
fn stored_key_rejects_padded_allowed_model_scope() {
    let token_hash =
        runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("secret").hash_base64();
    let record = RuntimeGatewayStoredVirtualKey {
        name: "alpha".to_string(),
        token_hash_base64: token_hash,
        virtual_key_id: Some(prodex_domain::VirtualKeyId::new().to_string()),
        tenant_id: Some("tenant-a".to_string()),
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        allowed_models: vec![" gpt-5 ".to_string()],
        budget_microusd: Some(1_000),
        request_budget: Some(10),
        rpm_limit: Some(5),
        tpm_limit: Some(500),
        disabled: Some(false),
        created_at_epoch: 1,
        updated_at_epoch: 2,
    };

    assert!(runtime_gateway_virtual_key_entry_from_stored(&record).is_none());
}

#[test]
fn stored_scim_user_auth_entry_rejects_padded_authz_fields() {
    let mut user = stored_scim_user_for_tests();
    user.tenant_id = Some(" tenant-a ".to_string());
    assert!(runtime_gateway_scim_user_auth_entry_from_stored(&user).is_none());

    let mut user = stored_scim_user_for_tests();
    user.allowed_key_prefixes = vec!["tenant-a- ".to_string()];
    assert!(runtime_gateway_scim_user_auth_entry_from_stored(&user).is_none());
}

#[test]
fn stored_scim_user_auth_entry_normalizes_empty_optional_scope_absent() {
    let mut user = stored_scim_user_for_tests();
    user.tenant_id = Some(String::new());

    let auth_user = runtime_gateway_scim_user_auth_entry_from_stored(&user).unwrap();

    assert_eq!(auth_user.tenant_id, None);
}

#[test]
fn stored_key_rejects_non_canonical_virtual_key_id() {
    let token_hash =
        runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("secret").hash_base64();
    let record = RuntimeGatewayStoredVirtualKey {
        name: "alpha".to_string(),
        token_hash_base64: token_hash,
        virtual_key_id: Some(" not-a-uuid ".to_string()),
        tenant_id: Some("tenant-a".to_string()),
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        allowed_models: vec!["gpt-5".to_string()],
        budget_microusd: Some(1_000),
        request_budget: Some(10),
        rpm_limit: Some(5),
        tpm_limit: Some(500),
        disabled: Some(false),
        created_at_epoch: 1,
        updated_at_epoch: 2,
    };

    assert!(runtime_gateway_virtual_key_entry_from_stored(&record).is_none());
}

#[test]
fn virtual_key_store_debug_output_redacts_sensitive_fields() {
    let id = prodex_domain::VirtualKeyId::new();
    let id_string = id.to_string();
    let stored = RuntimeGatewayStoredVirtualKey {
        name: "sk-store-secret".to_string(),
        token_hash_base64: "hash-store-secret".to_string(),
        virtual_key_id: Some(id_string.clone()),
        tenant_id: Some("tenant-store-secret".to_string()),
        team_id: Some("team-store-secret".to_string()),
        project_id: Some("project-store-secret".to_string()),
        user_id: Some("user-store-secret".to_string()),
        budget_id: Some("budget-store-secret".to_string()),
        allowed_models: vec!["model-store-secret".to_string()],
        budget_microusd: Some(10),
        request_budget: Some(20),
        rpm_limit: Some(30),
        tpm_limit: Some(40),
        disabled: Some(false),
        created_at_epoch: 50,
        updated_at_epoch: 60,
    };
    let scim_user = RuntimeGatewayScimUser {
        id: "scim-id-secret".to_string(),
        user_name: "scim-user-secret".to_string(),
        external_id: Some("external-secret".to_string()),
        display_name: Some("display-secret".to_string()),
        active: true,
        role: Some("admin".to_string()),
        tenant_id: Some("tenant-user-secret".to_string()),
        team_id: Some("team-user-secret".to_string()),
        project_id: Some("project-user-secret".to_string()),
        user_id: Some("user-user-secret".to_string()),
        group_ids: vec!["group-user-secret".to_string()],
        department_id: Some("department-user-secret".to_string()),
        budget_id: Some("budget-user-secret".to_string()),
        allowed_key_prefixes: vec!["prefix-secret".to_string()],
        created_at_epoch: 70,
        updated_at_epoch: 80,
    };
    let entry = RuntimeGatewayVirtualKeyEntry {
        virtual_key_id: Some(id),
        key: runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: "sk-entry-secret".to_string(),
            tenant_id: Some("tenant-entry-secret".to_string()),
            team_id: Some("team-entry-secret".to_string()),
            project_id: Some("project-entry-secret".to_string()),
            user_id: Some("user-entry-secret".to_string()),
            budget_id: Some("budget-entry-secret".to_string()),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                "entry-token-secret",
            ),
            allowed_models: vec!["model-entry-secret".to_string()],
            budget_microusd: Some(90),
            request_budget: Some(100),
            rpm_limit: Some(110),
            tpm_limit: Some(120),
        },
        source: RuntimeGatewayVirtualKeySource::Admin,
        tenant_id: Some("tenant-entry-secret".to_string()),
        group_ids: vec!["group-entry-secret".to_string()],
        department_id: Some("department-entry-secret".to_string()),
        created_at_epoch: Some(130),
        updated_at_epoch: Some(140),
        disabled: false,
    };
    let store = RuntimeGatewayVirtualKeyStoreFile {
        version: runtime_gateway_virtual_key_store_version(),
        keys: vec![stored.clone()],
        scim_users: vec![scim_user.clone()],
        ..RuntimeGatewayVirtualKeyStoreFile::default()
    };
    let rendered = format!("{stored:?}\n{scim_user:?}\n{entry:?}\n{store:?}");

    assert!(rendered.contains("RuntimeGatewayStoredVirtualKey"));
    assert!(rendered.contains("RuntimeGatewayScimUser"));
    assert!(rendered.contains("RuntimeGatewayVirtualKeyEntry"));
    assert!(rendered.contains("RuntimeGatewayVirtualKeyStoreFile"));
    assert!(rendered.contains("source: Admin"));
    assert!(rendered.contains("<redacted>"));
    for raw in [
        "sk-store-secret",
        "hash-store-secret",
        "tenant-store-secret",
        "team-store-secret",
        "project-store-secret",
        "user-store-secret",
        "budget-store-secret",
        "model-store-secret",
        "scim-id-secret",
        "scim-user-secret",
        "external-secret",
        "display-secret",
        "tenant-user-secret",
        "team-user-secret",
        "project-user-secret",
        "user-user-secret",
        "group-user-secret",
        "department-user-secret",
        "budget-user-secret",
        "prefix-secret",
        "sk-entry-secret",
        "tenant-entry-secret",
        "team-entry-secret",
        "project-entry-secret",
        "user-entry-secret",
        "group-entry-secret",
        "department-entry-secret",
        "budget-entry-secret",
        "entry-token-secret",
        "model-entry-secret",
        id_string.as_str(),
    ] {
        assert!(!rendered.contains(raw), "{rendered}");
    }
}
