use std::path::Path;

use anyhow::{Context, Result};
use postgres::GenericClient;
use redis::Commands;
use rusqlite::{Connection, params};

use super::local_rewrite_gateway_backend_connection::{
    runtime_gateway_postgres_open, runtime_gateway_redis_connection, runtime_gateway_sqlite_open,
};
use super::local_rewrite_gateway_sqlite_utils::{
    runtime_gateway_sqlite_i64_to_u64, runtime_gateway_sqlite_optional_i64_to_u64,
    runtime_gateway_sqlite_optional_u64_to_i64, runtime_gateway_sqlite_u64_to_i64,
};
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayScimUser, RuntimeGatewayStoredVirtualKey, RuntimeGatewayVirtualKeyStoreFile,
    runtime_gateway_virtual_key_store_version,
};

pub(super) fn runtime_gateway_sqlite_load_key_store(
    path: &Path,
) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let conn = runtime_gateway_sqlite_open(path)?;
    runtime_gateway_sqlite_load_key_store_from_conn(&conn)
}

pub(super) fn runtime_gateway_sqlite_load_key_store_from_conn(
    conn: &Connection,
) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let mut stmt = conn.prepare(
        r#"
        SELECT name, tenant_id, team_id, project_id, user_id, budget_id,
               token_hash_base64, allowed_models_json, budget_microusd,
               request_budget, rpm_limit, tpm_limit, disabled,
               created_at_epoch, updated_at_epoch
        FROM prodex_gateway_virtual_keys
        ORDER BY name COLLATE NOCASE
        "#,
    )?;
    let rows = stmt.query_map([], |row| {
        let allowed_models_json: String = row.get(7)?;
        let allowed_models =
            serde_json::from_str::<Vec<String>>(&allowed_models_json).unwrap_or_default();
        Ok(RuntimeGatewayStoredVirtualKey {
            name: row.get(0)?,
            tenant_id: row.get(1)?,
            team_id: row.get(2)?,
            project_id: row.get(3)?,
            user_id: row.get(4)?,
            budget_id: row.get(5)?,
            token_hash_base64: row.get(6)?,
            allowed_models,
            budget_microusd: runtime_gateway_sqlite_optional_i64_to_u64(row.get(8)?),
            request_budget: runtime_gateway_sqlite_optional_i64_to_u64(row.get(9)?),
            rpm_limit: runtime_gateway_sqlite_optional_i64_to_u64(row.get(10)?),
            tpm_limit: runtime_gateway_sqlite_optional_i64_to_u64(row.get(11)?),
            disabled: Some(row.get::<_, i64>(12)? != 0),
            created_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(13)?),
            updated_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(14)?),
        })
    })?;
    let mut keys = Vec::new();
    for row in rows {
        keys.push(row?);
    }
    Ok(RuntimeGatewayVirtualKeyStoreFile {
        version: runtime_gateway_virtual_key_store_version(),
        keys,
        scim_users: runtime_gateway_sqlite_load_scim_users_from_conn(conn)?,
    })
}

pub(super) fn runtime_gateway_postgres_load_key_store(
    url: &str,
) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let mut client = runtime_gateway_postgres_open(url)?;
    runtime_gateway_postgres_load_key_store_from_client(&mut client)
}

pub(super) fn runtime_gateway_postgres_load_key_store_from_client<C: GenericClient>(
    client: &mut C,
) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let rows = client.query(
        r#"
        SELECT name, tenant_id, team_id, project_id, user_id, budget_id,
               token_hash_base64, allowed_models_json, budget_microusd,
               request_budget, rpm_limit, tpm_limit, disabled,
               created_at_epoch, updated_at_epoch
        FROM prodex_gateway_virtual_keys
        ORDER BY lower(name), name
        "#,
        &[],
    )?;
    let mut keys = Vec::new();
    for row in rows {
        keys.push(runtime_gateway_postgres_stored_key_from_row(&row));
    }
    Ok(RuntimeGatewayVirtualKeyStoreFile {
        version: runtime_gateway_virtual_key_store_version(),
        keys,
        scim_users: runtime_gateway_postgres_load_scim_users_from_client(client)?,
    })
}

fn runtime_gateway_sqlite_load_scim_users_from_conn(
    conn: &Connection,
) -> Result<Vec<RuntimeGatewayScimUser>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, user_name, tenant_id, team_id, project_id, user_id, budget_id,
               external_id, display_name, active, role,
               allowed_key_prefixes_json, created_at_epoch, updated_at_epoch
        FROM prodex_gateway_scim_users
        ORDER BY user_name COLLATE NOCASE
        "#,
    )?;
    let rows = stmt.query_map([], |row| {
        let prefixes_json: String = row.get(11)?;
        let allowed_key_prefixes =
            serde_json::from_str::<Vec<String>>(&prefixes_json).unwrap_or_default();
        Ok(RuntimeGatewayScimUser {
            id: row.get(0)?,
            user_name: row.get(1)?,
            tenant_id: row.get(2)?,
            team_id: row.get(3)?,
            project_id: row.get(4)?,
            user_id: row.get(5)?,
            budget_id: row.get(6)?,
            external_id: row.get(7)?,
            display_name: row.get(8)?,
            active: row.get::<_, i64>(9)? != 0,
            role: row.get(10)?,
            allowed_key_prefixes,
            created_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(12)?),
            updated_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(13)?),
        })
    })?;
    let mut users = Vec::new();
    for row in rows {
        users.push(row?);
    }
    Ok(users)
}

fn runtime_gateway_postgres_load_scim_users_from_client<C: GenericClient>(
    client: &mut C,
) -> Result<Vec<RuntimeGatewayScimUser>> {
    let rows = client.query(
        r#"
        SELECT id, user_name, tenant_id, team_id, project_id, user_id, budget_id,
               external_id, display_name, active, role,
               allowed_key_prefixes_json, created_at_epoch, updated_at_epoch
        FROM prodex_gateway_scim_users
        ORDER BY lower(user_name), user_name
        "#,
        &[],
    )?;
    let mut users = Vec::new();
    for row in rows {
        let prefixes_json: String = row.get(11);
        users.push(RuntimeGatewayScimUser {
            id: row.get(0),
            user_name: row.get(1),
            tenant_id: row.get(2),
            team_id: row.get(3),
            project_id: row.get(4),
            user_id: row.get(5),
            budget_id: row.get(6),
            external_id: row.get(7),
            display_name: row.get(8),
            active: row.get::<_, bool>(9),
            role: row.get(10),
            allowed_key_prefixes: serde_json::from_str::<Vec<String>>(&prefixes_json)
                .unwrap_or_default(),
            created_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(12)),
            updated_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(13)),
        });
    }
    Ok(users)
}

fn runtime_gateway_postgres_stored_key_from_row(
    row: &postgres::Row,
) -> RuntimeGatewayStoredVirtualKey {
    let allowed_models_json: String = row.get(7);
    let allowed_models =
        serde_json::from_str::<Vec<String>>(&allowed_models_json).unwrap_or_default();
    RuntimeGatewayStoredVirtualKey {
        name: row.get(0),
        tenant_id: row.get(1),
        team_id: row.get(2),
        project_id: row.get(3),
        user_id: row.get(4),
        budget_id: row.get(5),
        token_hash_base64: row.get(6),
        allowed_models,
        budget_microusd: runtime_gateway_sqlite_optional_i64_to_u64(row.get(8)),
        request_budget: runtime_gateway_sqlite_optional_i64_to_u64(row.get(9)),
        rpm_limit: runtime_gateway_sqlite_optional_i64_to_u64(row.get(10)),
        tpm_limit: runtime_gateway_sqlite_optional_i64_to_u64(row.get(11)),
        disabled: Some(row.get::<_, bool>(12)),
        created_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(13)),
        updated_at_epoch: runtime_gateway_sqlite_i64_to_u64(row.get(14)),
    }
}

pub(super) fn runtime_gateway_redis_load_key_store(
    url: &str,
    redis_key: &str,
) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let mut conn = runtime_gateway_redis_connection(url)?;
    let payload: Option<String> = conn.get(redis_key)?;
    let Some(payload) = payload else {
        return Ok(RuntimeGatewayVirtualKeyStoreFile::default());
    };
    serde_json::from_str::<RuntimeGatewayVirtualKeyStoreFile>(&payload)
        .context("failed to parse gateway redis virtual key store")
}

pub(super) fn runtime_gateway_redis_save_key_store(
    conn: &mut redis::Connection,
    redis_key: &str,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> Result<()> {
    let payload = serde_json::to_string(store)?;
    let _: () = conn.set(redis_key, payload)?;
    Ok(())
}

pub(super) fn runtime_gateway_sqlite_save_key_store_in_tx(
    tx: &rusqlite::Transaction<'_>,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> Result<()> {
    tx.execute("DELETE FROM prodex_gateway_virtual_keys", [])?;
    for record in &store.keys {
        let allowed_models_json = serde_json::to_string(&record.allowed_models)?;
        tx.execute(
            r#"
            INSERT INTO prodex_gateway_virtual_keys (
                name, tenant_id, team_id, project_id, user_id, budget_id,
                token_hash_base64, allowed_models_json, budget_microusd,
                request_budget, rpm_limit, tpm_limit, disabled,
                created_at_epoch, updated_at_epoch
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
            params![
                record.name,
                record.tenant_id,
                record.team_id,
                record.project_id,
                record.user_id,
                record.budget_id,
                record.token_hash_base64,
                allowed_models_json,
                runtime_gateway_sqlite_optional_u64_to_i64(record.budget_microusd),
                runtime_gateway_sqlite_optional_u64_to_i64(record.request_budget),
                runtime_gateway_sqlite_optional_u64_to_i64(record.rpm_limit),
                runtime_gateway_sqlite_optional_u64_to_i64(record.tpm_limit),
                if record.disabled.unwrap_or(false) {
                    1_i64
                } else {
                    0_i64
                },
                runtime_gateway_sqlite_u64_to_i64(record.created_at_epoch),
                runtime_gateway_sqlite_u64_to_i64(record.updated_at_epoch),
            ],
        )?;
    }
    tx.execute("DELETE FROM prodex_gateway_scim_users", [])?;
    for user in &store.scim_users {
        let allowed_key_prefixes_json = serde_json::to_string(&user.allowed_key_prefixes)?;
        tx.execute(
            r#"
            INSERT INTO prodex_gateway_scim_users (
                id, user_name, tenant_id, team_id, project_id, user_id, budget_id,
                external_id, display_name, active, role,
                allowed_key_prefixes_json, created_at_epoch, updated_at_epoch
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            "#,
            params![
                user.id,
                user.user_name,
                user.tenant_id,
                user.team_id,
                user.project_id,
                user.user_id,
                user.budget_id,
                user.external_id,
                user.display_name,
                if user.active { 1_i64 } else { 0_i64 },
                user.role,
                allowed_key_prefixes_json,
                runtime_gateway_sqlite_u64_to_i64(user.created_at_epoch),
                runtime_gateway_sqlite_u64_to_i64(user.updated_at_epoch),
            ],
        )?;
    }
    Ok(())
}

pub(super) fn runtime_gateway_postgres_save_key_store_in_tx(
    tx: &mut postgres::Transaction<'_>,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> Result<()> {
    tx.execute("DELETE FROM prodex_gateway_virtual_keys", &[])?;
    for record in &store.keys {
        let allowed_models_json = serde_json::to_string(&record.allowed_models)?;
        let disabled = record.disabled.unwrap_or(false);
        tx.execute(
            r#"
            INSERT INTO prodex_gateway_virtual_keys (
                name, tenant_id, team_id, project_id, user_id, budget_id,
                token_hash_base64, allowed_models_json, budget_microusd,
                request_budget, rpm_limit, tpm_limit, disabled,
                created_at_epoch, updated_at_epoch
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            "#,
            &[
                &record.name,
                &record.tenant_id,
                &record.team_id,
                &record.project_id,
                &record.user_id,
                &record.budget_id,
                &record.token_hash_base64,
                &allowed_models_json,
                &runtime_gateway_sqlite_optional_u64_to_i64(record.budget_microusd),
                &runtime_gateway_sqlite_optional_u64_to_i64(record.request_budget),
                &runtime_gateway_sqlite_optional_u64_to_i64(record.rpm_limit),
                &runtime_gateway_sqlite_optional_u64_to_i64(record.tpm_limit),
                &disabled,
                &runtime_gateway_sqlite_u64_to_i64(record.created_at_epoch),
                &runtime_gateway_sqlite_u64_to_i64(record.updated_at_epoch),
            ],
        )?;
    }
    tx.execute("DELETE FROM prodex_gateway_scim_users", &[])?;
    for user in &store.scim_users {
        let allowed_key_prefixes_json = serde_json::to_string(&user.allowed_key_prefixes)?;
        tx.execute(
            r#"
            INSERT INTO prodex_gateway_scim_users (
                id, user_name, tenant_id, team_id, project_id, user_id, budget_id,
                external_id, display_name, active, role,
                allowed_key_prefixes_json, created_at_epoch, updated_at_epoch
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
            &[
                &user.id,
                &user.user_name,
                &user.tenant_id,
                &user.team_id,
                &user.project_id,
                &user.user_id,
                &user.budget_id,
                &user.external_id,
                &user.display_name,
                &user.active,
                &user.role,
                &allowed_key_prefixes_json,
                &runtime_gateway_sqlite_u64_to_i64(user.created_at_epoch),
                &runtime_gateway_sqlite_u64_to_i64(user.updated_at_epoch),
            ],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("prodex-gateway-key-store-{name}-{stamp}"))
    }

    #[test]
    fn sqlite_key_store_round_trips_keys_and_scim_users() {
        let root = temp_dir("sqlite");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("state.sqlite");
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
                budget_id: None,
                allowed_key_prefixes: vec!["alpha".to_string()],
                created_at_epoch: 3,
                updated_at_epoch: 4,
            }],
        };
        runtime_gateway_sqlite_save_key_store_in_tx(&tx, &store).unwrap();
        tx.commit().unwrap();

        let loaded = runtime_gateway_sqlite_load_key_store(&path).unwrap();
        assert_eq!(loaded.keys[0].name, "alpha");
        assert_eq!(loaded.scim_users[0].user_name, "user@example.com");

        std::fs::remove_dir_all(root).unwrap();
    }
}
