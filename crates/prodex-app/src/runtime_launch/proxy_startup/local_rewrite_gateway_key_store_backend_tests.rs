use super::super::local_rewrite_gateway_backend_connection::{
    runtime_gateway_postgres_migrate_compatibility_state,
    runtime_gateway_postgres_migrate_enterprise_state,
    runtime_gateway_sqlite_create_current_schema_for_tests,
};
use super::{
    RuntimeGatewayScimUser, RuntimeGatewayStoredVirtualKey, RuntimeGatewayVirtualKeyStoreFile,
    runtime_gateway_exact_json_vec_for_field, runtime_gateway_postgres_load_key_store,
    runtime_gateway_postgres_open, runtime_gateway_postgres_save_key_store_in_tx,
    runtime_gateway_redis_hash_exact_json_vec, runtime_gateway_redis_hash_exact_string,
    runtime_gateway_redis_hash_optional_u64, runtime_gateway_redis_scim_user_from_hash,
    runtime_gateway_redis_stored_key_from_hash, runtime_gateway_sql_key_store_i64_to_u64,
    runtime_gateway_sqlite_load_key_store, runtime_gateway_sqlite_open,
    runtime_gateway_sqlite_save_key_store_in_tx, runtime_gateway_virtual_key_store_version,
};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("prodex-gateway-key-store-{name}-{stamp}"))
}

fn runtime_gateway_postgres_create_current_schema_for_tests(url: &str) {
    let tls = prodex_storage_postgres_runtime::PostgresTlsConfig::explicit_disable();
    runtime_gateway_postgres_migrate_enterprise_state(url, &tls)
        .expect("postgres enterprise migrations should apply");
    runtime_gateway_postgres_migrate_compatibility_state(url, &tls)
        .expect("postgres compatibility migrations should apply");
}

#[test]
fn redis_hash_exact_string_rejects_trim_normalization() {
    let fields = BTreeMap::from([
        ("canonical".to_string(), "team-a".to_string()),
        ("padded".to_string(), " team-a ".to_string()),
        ("empty".to_string(), String::new()),
    ]);

    assert_eq!(
        runtime_gateway_redis_hash_exact_string(&fields, "canonical"),
        Some("team-a".to_string())
    );
    assert_eq!(
        runtime_gateway_redis_hash_exact_string(&fields, "padded"),
        None
    );
    assert_eq!(
        runtime_gateway_redis_hash_exact_string(&fields, "empty"),
        None
    );
}

#[test]
fn redis_stored_key_rejects_padded_identity_fields() {
    let fields = BTreeMap::from([
        ("name".to_string(), " key-a ".to_string()),
        ("token_hash_base64".to_string(), "hash".to_string()),
        ("allowed_models_json".to_string(), "[]".to_string()),
    ]);

    assert!(
        runtime_gateway_redis_stored_key_from_hash(&fields)
            .unwrap()
            .is_none()
    );
}

#[test]
fn redis_exact_json_vec_rejects_whitespace_entries() {
    let fields = BTreeMap::from([
        (
            "canonical".to_string(),
            serde_json::json!(["gpt-5", "gpt-5-mini"]).to_string(),
        ),
        (
            "padded".to_string(),
            serde_json::json!(["gpt-5", " gpt-5-mini "]).to_string(),
        ),
    ]);

    assert_eq!(
        runtime_gateway_redis_hash_exact_json_vec(&fields, "canonical").unwrap(),
        vec!["gpt-5".to_string(), "gpt-5-mini".to_string()]
    );
    assert!(
        runtime_gateway_redis_hash_exact_json_vec(&fields, "padded")
            .unwrap_err()
            .to_string()
            .contains("gateway redis key-store field padded entries must not contain whitespace")
    );
}

#[test]
fn exact_json_vec_rejects_whitespace_entries_across_backends() {
    assert_eq!(
        runtime_gateway_exact_json_vec_for_field(
            r#"["gpt-5","gpt-5-mini"]"#,
            "gateway key-store field allowed_models_json"
        )
        .unwrap(),
        vec!["gpt-5".to_string(), "gpt-5-mini".to_string()]
    );
    assert!(
        runtime_gateway_exact_json_vec_for_field(
            r#"["gpt-5"," gpt-5-mini "]"#,
            "gateway key-store field allowed_models_json"
        )
        .unwrap_err()
        .to_string()
        .contains(
            "gateway key-store field allowed_models_json entries must not contain whitespace"
        )
    );
    assert!(
        runtime_gateway_exact_json_vec_for_field(
            r#"[""]"#,
            "gateway key-store field allowed_models_json"
        )
        .unwrap_err()
        .to_string()
        .contains("gateway key-store field allowed_models_json must not contain empty entries")
    );
}

#[test]
fn sqlite_key_store_rejects_malformed_allowed_models_json() {
    let root = temp_dir("sqlite-bad-json-array");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&path).unwrap();
    let conn = runtime_gateway_sqlite_open(&path).unwrap();
    conn.execute(
        r#"
            INSERT INTO prodex_gateway_virtual_keys (
                name, token_hash_base64, allowed_models_json,
                disabled, created_at_epoch, updated_at_epoch
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        rusqlite::params![
            "alpha",
            "hash",
            r#"["gpt-5"," gpt-5-mini "]"#,
            0_i64,
            1_i64,
            2_i64
        ],
    )
    .unwrap();

    let err = runtime_gateway_sqlite_load_key_store(&path).unwrap_err();

    assert!(
        err.to_string()
            .contains("Conversion error from type Text at index: 8"),
        "{err:?}"
    );

    conn.execute(
        "UPDATE prodex_gateway_virtual_keys SET allowed_models_json = '{'",
        [],
    )
    .unwrap();
    assert!(runtime_gateway_sqlite_load_key_store(&path).is_err());

    conn.execute(
        "UPDATE prodex_gateway_virtual_keys SET allowed_models_json = '[\"gpt-5\"]'",
        [],
    )
    .unwrap();
    conn.execute(
        r#"
            INSERT INTO prodex_gateway_scim_users (
                id, user_name, active, allowed_key_prefixes_json,
                created_at_epoch, updated_at_epoch
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        rusqlite::params!["user-1", "user@example.com", 1_i64, "{", 1_i64, 2_i64],
    )
    .unwrap();
    assert!(runtime_gateway_sqlite_load_key_store(&path).is_err());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_key_store_rejects_malformed_boolean_fields() {
    let root = temp_dir("sqlite-bad-bool");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&path).unwrap();
    let conn = runtime_gateway_sqlite_open(&path).unwrap();
    conn.execute(
        r#"
            INSERT INTO prodex_gateway_virtual_keys (
                name, token_hash_base64, allowed_models_json,
                disabled, created_at_epoch, updated_at_epoch
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        rusqlite::params!["alpha", "hash", "[]", 2_i64, 1_i64, 2_i64],
    )
    .unwrap();

    let err = runtime_gateway_sqlite_load_key_store(&path).unwrap_err();
    assert!(
        err.to_string()
            .contains("Conversion error from type Integer at index: 13"),
        "{err:?}"
    );

    conn.execute("DELETE FROM prodex_gateway_virtual_keys", [])
        .unwrap();
    conn.execute(
        r#"
            INSERT INTO prodex_gateway_scim_users (
                id, user_name, active, allowed_key_prefixes_json,
                created_at_epoch, updated_at_epoch
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        rusqlite::params!["user-1", "user@example.com", 2_i64, "[]", 1_i64, 2_i64],
    )
    .unwrap();

    let err = runtime_gateway_sqlite_load_key_store(&path).unwrap_err();
    assert!(
        err.to_string()
            .contains("Conversion error from type Integer at index: 9"),
        "{err:?}"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_key_store_rejects_negative_numeric_fields() {
    let root = temp_dir("sqlite-negative-numeric");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&path).unwrap();
    let conn = runtime_gateway_sqlite_open(&path).unwrap();
    conn.execute(
        r#"
            INSERT INTO prodex_gateway_virtual_keys (
                name, token_hash_base64, allowed_models_json, budget_microusd,
                disabled, created_at_epoch, updated_at_epoch
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        rusqlite::params!["alpha", "hash", "[]", -1_i64, 0_i64, 1_i64, 2_i64],
    )
    .unwrap();

    let err = runtime_gateway_sqlite_load_key_store(&path).unwrap_err();
    assert!(
        err.to_string()
            .contains("Conversion error from type Integer at index: 9"),
        "{err:?}"
    );

    conn.execute("DELETE FROM prodex_gateway_virtual_keys", [])
        .unwrap();
    conn.execute(
        r#"
            INSERT INTO prodex_gateway_scim_users (
                id, user_name, active, allowed_key_prefixes_json,
                created_at_epoch, updated_at_epoch
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        rusqlite::params!["user-1", "user@example.com", 1_i64, "[]", -1_i64, 2_i64],
    )
    .unwrap();

    let err = runtime_gateway_sqlite_load_key_store(&path).unwrap_err();
    assert!(
        err.to_string()
            .contains("Conversion error from type Integer at index: 14"),
        "{err:?}"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn postgres_key_store_rejects_negative_numeric_fields() {
    let err = runtime_gateway_sql_key_store_i64_to_u64(-1, "budget_microusd").unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway SQL key-store field budget_microusd must be non-negative"),
        "{err:?}"
    );
}

#[test]
fn sqlite_key_store_round_trips_keys_and_scim_users() {
    let root = temp_dir("sqlite");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite");
    runtime_gateway_sqlite_create_current_schema_for_tests(&path).unwrap();
    let mut conn = runtime_gateway_sqlite_open(&path).unwrap();
    let tx = conn.transaction().unwrap();
    let store = RuntimeGatewayVirtualKeyStoreFile {
        version: runtime_gateway_virtual_key_store_version(),
        keys: vec![RuntimeGatewayStoredVirtualKey {
            name: "alpha".to_string(),
            token_hash_base64: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                "secret",
            )
            .hash_base64(),
            virtual_key_id: Some(prodex_domain::VirtualKeyId::new().to_string()),
            tenant_id: Some("tenant".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_models: vec!["gpt-5".to_string()],
            budget_microusd: Some(100),
            request_budget: Some(2),
            rpm_limit: None,
            tpm_limit: None,
            disabled: Some(false),
            created_at_epoch: 1,
            updated_at_epoch: 2,
        }],
        scim_users: vec![RuntimeGatewayScimUser {
            id: "user-1".to_string(),
            user_name: "user@example.com".to_string(),
            external_id: None,
            display_name: Some("User".to_string()),
            active: true,
            role: Some("admin".to_string()),
            tenant_id: Some("tenant".to_string()),
            team_id: None,
            project_id: None,
            user_id: Some("user-1".to_string()),
            group_ids: vec!["engineering".to_string()],
            department_id: Some("research".to_string()),
            budget_id: None,
            allowed_key_prefixes: vec!["alpha".to_string()],
            created_at_epoch: 3,
            updated_at_epoch: 4,
        }],
        ..RuntimeGatewayVirtualKeyStoreFile::default()
    };
    runtime_gateway_sqlite_save_key_store_in_tx(&tx, &store).unwrap();
    tx.commit().unwrap();

    let loaded = runtime_gateway_sqlite_load_key_store(&path).unwrap();
    assert_eq!(loaded.keys[0].name, "alpha");
    assert!(loaded.keys[0].virtual_key_id.is_some());
    assert_eq!(loaded.scim_users[0].user_name, "user@example.com");
    assert_eq!(loaded.scim_users[0].group_ids, ["engineering"]);
    assert_eq!(
        loaded.scim_users[0].department_id.as_deref(),
        Some("research")
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn postgres_key_store_round_trips_keys_and_scim_users() {
    let Some(url) = std::env::var("PRODEX_TEST_POSTGRES_URL").ok() else {
        eprintln!("skipping: PRODEX_TEST_POSTGRES_URL is not set");
        return;
    };
    runtime_gateway_postgres_create_current_schema_for_tests(&url);
    let tenant_id = prodex_domain::TenantId::new().to_string();
    let store = RuntimeGatewayVirtualKeyStoreFile {
        version: runtime_gateway_virtual_key_store_version(),
        keys: vec![RuntimeGatewayStoredVirtualKey {
            name: format!("alpha-{}", prodex_domain::VirtualKeyId::new()),
            token_hash_base64: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(
                "secret",
            )
            .hash_base64(),
            virtual_key_id: Some(prodex_domain::VirtualKeyId::new().to_string()),
            tenant_id: Some(tenant_id.clone()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_models: vec!["gpt-5".to_string()],
            budget_microusd: Some(100),
            request_budget: Some(2),
            rpm_limit: None,
            tpm_limit: None,
            disabled: Some(false),
            created_at_epoch: 1,
            updated_at_epoch: 2,
        }],
        scim_users: vec![RuntimeGatewayScimUser {
            id: format!("user-{}", prodex_domain::PrincipalId::new()),
            user_name: format!("user-{}@example.com", prodex_domain::PrincipalId::new()),
            external_id: None,
            display_name: Some("User".to_string()),
            active: true,
            role: Some("admin".to_string()),
            tenant_id: Some(tenant_id),
            team_id: None,
            project_id: None,
            user_id: Some("user-1".to_string()),
            group_ids: vec!["engineering".to_string()],
            department_id: Some("research".to_string()),
            budget_id: None,
            allowed_key_prefixes: vec!["alpha".to_string()],
            created_at_epoch: 3,
            updated_at_epoch: 4,
        }],
        ..RuntimeGatewayVirtualKeyStoreFile::default()
    };

    let tls = prodex_storage_postgres_runtime::PostgresTlsConfig::explicit_disable();
    let mut conn = runtime_gateway_postgres_open(&url, &tls).unwrap();
    let mut tx = conn.transaction().unwrap();
    if let Err(error) = runtime_gateway_postgres_save_key_store_in_tx(&mut tx, &store) {
        panic!("{error:#}");
    }
    tx.commit().unwrap();

    let loaded = runtime_gateway_postgres_load_key_store(&url, &tls).unwrap();
    assert!(loaded.keys.iter().any(|key| key.name == store.keys[0].name));
    assert!(
        loaded
            .keys
            .iter()
            .any(|key| key.virtual_key_id == store.keys[0].virtual_key_id)
    );
    assert!(loaded.scim_users.iter().any(|user| {
        user.user_name == store.scim_users[0].user_name
            && user.group_ids == ["engineering"]
            && user.department_id.as_deref() == Some("research")
    }));
}

#[test]
fn redis_key_store_backend_does_not_write_whole_store_json_blob() {
    let source = include_str!("local_rewrite_gateway_key_store_backend.rs");
    let set_blob = ["conn.set", "(redis_key"].join("");
    let whole_store_json = ["serde_json::to_string", "(store"].join("");
    assert!(!source.contains(&set_blob));
    assert!(!source.contains(&whole_store_json));
}

#[test]
fn redis_key_store_optional_u64_preserves_zero() {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert("zero".to_string(), "0".to_string());
    fields.insert("missing".to_string(), String::new());

    assert_eq!(
        runtime_gateway_redis_hash_optional_u64(&fields, "zero").unwrap(),
        Some(0)
    );
    assert_eq!(
        runtime_gateway_redis_hash_optional_u64(&fields, "missing").unwrap(),
        None
    );
}

#[test]
fn redis_key_store_hash_rejects_malformed_numeric_fields() {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert("name".to_string(), "alpha".to_string());
    fields.insert("token_hash_base64".to_string(), "hash".to_string());
    fields.insert("created_at_epoch".to_string(), "1".to_string());
    fields.insert("updated_at_epoch".to_string(), "2".to_string());
    fields.insert("budget_microusd".to_string(), "not-a-number".to_string());

    let err = runtime_gateway_redis_stored_key_from_hash(&fields).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway redis key-store field budget_microusd must be an unsigned integer"),
        "{err:?}"
    );

    fields.insert("budget_microusd".to_string(), "100".to_string());
    fields.insert("updated_at_epoch".to_string(), " 2 ".to_string());
    let err = runtime_gateway_redis_stored_key_from_hash(&fields).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway redis key-store field updated_at_epoch must not contain whitespace"),
        "{err:?}"
    );
}

#[test]
fn redis_key_store_hash_rejects_malformed_bool_fields() {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert("name".to_string(), "alpha".to_string());
    fields.insert("token_hash_base64".to_string(), "hash".to_string());
    fields.insert("created_at_epoch".to_string(), "1".to_string());
    fields.insert("updated_at_epoch".to_string(), "2".to_string());
    fields.insert("disabled".to_string(), "false".to_string());

    let err = runtime_gateway_redis_stored_key_from_hash(&fields).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway redis key-store field disabled must be 0 or 1"),
        "{err:?}"
    );

    let mut user_fields = std::collections::BTreeMap::new();
    user_fields.insert("id".to_string(), "user-1".to_string());
    user_fields.insert("user_name".to_string(), "user@example.com".to_string());
    user_fields.insert("created_at_epoch".to_string(), "1".to_string());
    user_fields.insert("updated_at_epoch".to_string(), "2".to_string());
    user_fields.insert("active".to_string(), " 1 ".to_string());

    let err = runtime_gateway_redis_scim_user_from_hash(&user_fields).unwrap_err();
    assert!(
        err.to_string()
            .contains("gateway redis key-store field active must not contain whitespace"),
        "{err:?}"
    );
}

#[test]
fn redis_key_store_hash_rejects_malformed_json_array_fields() {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert("name".to_string(), "alpha".to_string());
    fields.insert("token_hash_base64".to_string(), "hash".to_string());
    fields.insert("created_at_epoch".to_string(), "1".to_string());
    fields.insert("updated_at_epoch".to_string(), "2".to_string());
    fields.insert(
        "allowed_models_json".to_string(),
        serde_json::json!(["gpt-5", " gpt-5-mini "]).to_string(),
    );

    let err = runtime_gateway_redis_stored_key_from_hash(&fields).unwrap_err();
    assert!(
        err.to_string().contains(
            "gateway redis key-store field allowed_models_json entries must not contain whitespace"
        ),
        "{err:?}"
    );

    let mut user_fields = std::collections::BTreeMap::new();
    user_fields.insert("id".to_string(), "user-1".to_string());
    user_fields.insert("user_name".to_string(), "user@example.com".to_string());
    user_fields.insert("created_at_epoch".to_string(), "1".to_string());
    user_fields.insert("updated_at_epoch".to_string(), "2".to_string());
    user_fields.insert("allowed_key_prefixes_json".to_string(), "{".to_string());

    let err = runtime_gateway_redis_scim_user_from_hash(&user_fields).unwrap_err();
    assert!(
            err.to_string().contains(
                "gateway redis key-store field allowed_key_prefixes_json must be a JSON array of strings"
            ),
            "{err:?}"
        );
}
