use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result};
use postgres::GenericClient;
use prodex_domain::TenantId;
use redis::Commands;
use rusqlite::{Connection, params};

use super::local_rewrite_gateway_backend_connection::{
    runtime_gateway_postgres_open, runtime_gateway_redis_connection, runtime_gateway_sqlite_open,
};
use super::local_rewrite_gateway_sqlite_utils::{
    runtime_gateway_sqlite_optional_u64_to_i64, runtime_gateway_sqlite_u64_to_i64,
};
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayScimUser, RuntimeGatewayStoredVirtualKey, RuntimeGatewayVirtualKeyStoreFile,
    runtime_gateway_virtual_key_store_version,
};

fn runtime_gateway_typed_tenant_ids(
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> std::collections::BTreeSet<TenantId> {
    store
        .keys
        .iter()
        .filter_map(|record| record.tenant_id.as_deref())
        .chain(
            store
                .scim_users
                .iter()
                .filter_map(|user| user.tenant_id.as_deref()),
        )
        .filter_map(|tenant_id| TenantId::from_str(tenant_id).ok())
        .collect()
}

fn runtime_gateway_sqlite_upsert_tenants_in_tx(
    tx: &rusqlite::Transaction<'_>,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> Result<()> {
    for tenant_id in runtime_gateway_typed_tenant_ids(store) {
        let tenant_id = tenant_id.to_string();
        let now = runtime_gateway_sqlite_u64_to_i64(
            store
                .keys
                .iter()
                .filter_map(|record| record.updated_at_epoch.checked_mul(1000))
                .chain(
                    store
                        .scim_users
                        .iter()
                        .filter_map(|user| user.updated_at_epoch.checked_mul(1000)),
                )
                .max()
                .unwrap_or(0),
        );
        tx.execute(
            r#"
            INSERT INTO prodex_tenants (
                tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms
            ) VALUES (?1, ?2, ?3, ?3)
            ON CONFLICT(tenant_id) DO UPDATE SET
                display_name = excluded.display_name,
                updated_at_unix_ms = excluded.updated_at_unix_ms
            WHERE prodex_tenants.tenant_id = excluded.tenant_id
            "#,
            params![tenant_id, tenant_id, now],
        )?;
    }
    Ok(())
}

fn runtime_gateway_postgres_upsert_tenants_in_tx(
    tx: &mut postgres::Transaction<'_>,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> Result<()> {
    for tenant_id in runtime_gateway_typed_tenant_ids(store) {
        let tenant_uuid = tenant_id.as_uuid();
        let tenant_display_name = tenant_id.to_string();
        let now = runtime_gateway_sqlite_u64_to_i64(
            store
                .keys
                .iter()
                .filter_map(|record| record.updated_at_epoch.checked_mul(1000))
                .chain(
                    store
                        .scim_users
                        .iter()
                        .filter_map(|user| user.updated_at_epoch.checked_mul(1000)),
                )
                .max()
                .unwrap_or(0),
        );
        tx.execute(
            r#"
            INSERT INTO prodex_tenants (
                tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms
            ) VALUES ($1, $2, $3, $3)
            ON CONFLICT (tenant_id) DO UPDATE SET
                display_name = EXCLUDED.display_name,
                updated_at_unix_ms = EXCLUDED.updated_at_unix_ms
            WHERE prodex_tenants.tenant_id = EXCLUDED.tenant_id
            "#,
            &[&tenant_uuid, &tenant_display_name, &now],
        )?;
    }
    Ok(())
}

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
               token_hash_base64, virtual_key_id, allowed_models_json, budget_microusd,
               request_budget, rpm_limit, tpm_limit, disabled, created_at_epoch, updated_at_epoch
        FROM prodex_gateway_virtual_keys
        ORDER BY name COLLATE NOCASE
        "#,
    )?;
    let rows = stmt.query_map([], |row| {
        let allowed_models_json: String = row.get(8)?;
        let allowed_models = runtime_gateway_exact_json_vec_for_field(
            &allowed_models_json,
            "gateway key-store field allowed_models_json",
        )
        .map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(
                8,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    err.to_string(),
                )),
            )
        })?;
        Ok(RuntimeGatewayStoredVirtualKey {
            name: row.get(0)?,
            tenant_id: row.get(1)?,
            team_id: row.get(2)?,
            project_id: row.get(3)?,
            user_id: row.get(4)?,
            budget_id: row.get(5)?,
            token_hash_base64: row.get(6)?,
            virtual_key_id: row.get(7)?,
            allowed_models,
            budget_microusd: runtime_gateway_sqlite_key_store_optional_u64(
                row.get(9)?,
                9,
                "budget_microusd",
            )?,
            request_budget: runtime_gateway_sqlite_key_store_optional_u64(
                row.get(10)?,
                10,
                "request_budget",
            )?,
            rpm_limit: runtime_gateway_sqlite_key_store_optional_u64(
                row.get(11)?,
                11,
                "rpm_limit",
            )?,
            tpm_limit: runtime_gateway_sqlite_key_store_optional_u64(
                row.get(12)?,
                12,
                "tpm_limit",
            )?,
            disabled: Some(runtime_gateway_sqlite_exact_bool(
                row.get(13)?,
                13,
                "disabled",
            )?),
            created_at_epoch: runtime_gateway_sqlite_key_store_u64(
                row.get(14)?,
                14,
                "created_at_epoch",
            )?,
            updated_at_epoch: runtime_gateway_sqlite_key_store_u64(
                row.get(15)?,
                15,
                "updated_at_epoch",
            )?,
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

fn runtime_gateway_sqlite_exact_bool(
    value: i64,
    column: usize,
    field: &str,
) -> rusqlite::Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Integer,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("gateway sqlite key-store field {field} must be 0 or 1"),
            )),
        )),
    }
}

fn runtime_gateway_sqlite_key_store_optional_u64(
    value: Option<i64>,
    column: usize,
    field: &str,
) -> rusqlite::Result<Option<u64>> {
    value
        .map(|value| runtime_gateway_sqlite_key_store_u64(value, column, field))
        .transpose()
}

fn runtime_gateway_sqlite_key_store_u64(
    value: i64,
    column: usize,
    field: &str,
) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Integer,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("gateway sqlite key-store field {field} must be non-negative"),
            )),
        )
    })
}

fn runtime_gateway_sql_key_store_optional_i64_to_u64(
    value: Option<i64>,
    field: &str,
) -> Result<Option<u64>> {
    value
        .map(|value| runtime_gateway_sql_key_store_i64_to_u64(value, field))
        .transpose()
}

fn runtime_gateway_sql_key_store_i64_to_u64(value: i64, field: &str) -> Result<u64> {
    u64::try_from(value)
        .with_context(|| format!("gateway SQL key-store field {field} must be non-negative"))
}

pub(super) fn runtime_gateway_postgres_load_key_store(
    url: &str,
    tls: &prodex_storage_postgres_runtime::PostgresTlsConfig,
) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let mut client = runtime_gateway_postgres_open(url, tls)?;
    runtime_gateway_postgres_load_key_store_from_client(&mut client)
}

pub(super) fn runtime_gateway_postgres_load_key_store_from_client<C: GenericClient>(
    client: &mut C,
) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let rows = client.query(
        r#"
        SELECT name, tenant_id, team_id, project_id, user_id, budget_id,
               token_hash_base64, virtual_key_id, allowed_models_json, budget_microusd,
               request_budget, rpm_limit, tpm_limit, disabled, created_at_epoch, updated_at_epoch
        FROM prodex_gateway_virtual_keys
        ORDER BY lower(name), name
        "#,
        &[],
    )?;
    let mut keys = Vec::new();
    for row in rows {
        keys.push(runtime_gateway_postgres_stored_key_from_row(&row)?);
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
        let allowed_key_prefixes = runtime_gateway_exact_json_vec_for_field(
            &prefixes_json,
            "gateway key-store field allowed_key_prefixes_json",
        )
        .map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(
                11,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    err.to_string(),
                )),
            )
        })?;
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
            active: runtime_gateway_sqlite_exact_bool(row.get(9)?, 9, "active")?,
            role: row.get(10)?,
            allowed_key_prefixes,
            created_at_epoch: runtime_gateway_sqlite_key_store_u64(
                row.get(12)?,
                12,
                "created_at_epoch",
            )?,
            updated_at_epoch: runtime_gateway_sqlite_key_store_u64(
                row.get(13)?,
                13,
                "updated_at_epoch",
            )?,
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
            allowed_key_prefixes: runtime_gateway_exact_json_vec_for_field(
                &prefixes_json,
                "gateway key-store field allowed_key_prefixes_json",
            )?,
            created_at_epoch: runtime_gateway_sql_key_store_i64_to_u64(
                row.get(12),
                "created_at_epoch",
            )?,
            updated_at_epoch: runtime_gateway_sql_key_store_i64_to_u64(
                row.get(13),
                "updated_at_epoch",
            )?,
        });
    }
    Ok(users)
}

fn runtime_gateway_postgres_stored_key_from_row(
    row: &postgres::Row,
) -> Result<RuntimeGatewayStoredVirtualKey> {
    let allowed_models_json: String = row.get(8);
    let allowed_models = runtime_gateway_exact_json_vec_for_field(
        &allowed_models_json,
        "gateway key-store field allowed_models_json",
    )?;
    Ok(RuntimeGatewayStoredVirtualKey {
        name: row.get(0),
        tenant_id: row.get(1),
        team_id: row.get(2),
        project_id: row.get(3),
        user_id: row.get(4),
        budget_id: row.get(5),
        token_hash_base64: row.get(6),
        virtual_key_id: row.get(7),
        allowed_models,
        budget_microusd: runtime_gateway_sql_key_store_optional_i64_to_u64(
            row.get(9),
            "budget_microusd",
        )?,
        request_budget: runtime_gateway_sql_key_store_optional_i64_to_u64(
            row.get(10),
            "request_budget",
        )?,
        rpm_limit: runtime_gateway_sql_key_store_optional_i64_to_u64(row.get(11), "rpm_limit")?,
        tpm_limit: runtime_gateway_sql_key_store_optional_i64_to_u64(row.get(12), "tpm_limit")?,
        disabled: Some(row.get::<_, bool>(13)),
        created_at_epoch: runtime_gateway_sql_key_store_i64_to_u64(
            row.get(14),
            "created_at_epoch",
        )?,
        updated_at_epoch: runtime_gateway_sql_key_store_i64_to_u64(
            row.get(15),
            "updated_at_epoch",
        )?,
    })
}

pub(super) fn runtime_gateway_redis_load_key_store(
    url: &str,
    redis_key: &str,
) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let mut conn = runtime_gateway_redis_connection(url)?;
    runtime_gateway_redis_load_key_store_from_conn(&mut conn, redis_key)
}

pub(super) fn runtime_gateway_redis_load_key_store_from_conn(
    conn: &mut redis::Connection,
    redis_key: &str,
) -> Result<RuntimeGatewayVirtualKeyStoreFile> {
    let key_index = runtime_gateway_redis_key_store_key_index(redis_key);
    let names: Vec<String> = conn.smembers(&key_index)?;
    if names.is_empty() {
        let payload: Option<String> = conn.get(redis_key)?;
        let Some(payload) = payload else {
            return Ok(RuntimeGatewayVirtualKeyStoreFile::default());
        };
        return serde_json::from_str::<RuntimeGatewayVirtualKeyStoreFile>(&payload)
            .context("failed to parse legacy gateway redis virtual key store");
    }

    let mut keys = Vec::new();
    for name in names {
        let fields: std::collections::BTreeMap<String, String> =
            conn.hgetall(runtime_gateway_redis_key_store_key_hash(redis_key, &name))?;
        if let Some(record) = runtime_gateway_redis_stored_key_from_hash(&fields)? {
            keys.push(record);
        }
    }

    let user_index = runtime_gateway_redis_key_store_scim_index(redis_key);
    let user_ids: Vec<String> = conn.smembers(&user_index)?;
    let mut scim_users = Vec::new();
    for id in user_ids {
        let fields: std::collections::BTreeMap<String, String> =
            conn.hgetall(runtime_gateway_redis_key_store_scim_hash(redis_key, &id))?;
        if let Some(user) = runtime_gateway_redis_scim_user_from_hash(&fields)? {
            scim_users.push(user);
        }
    }

    Ok(RuntimeGatewayVirtualKeyStoreFile {
        version: runtime_gateway_virtual_key_store_version(),
        keys,
        scim_users,
    })
}

pub(super) fn runtime_gateway_redis_save_key_store(
    conn: &mut redis::Connection,
    redis_key: &str,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> Result<()> {
    let key_index = runtime_gateway_redis_key_store_key_index(redis_key);
    let old_names: Vec<String> = conn.smembers(&key_index)?;
    for name in old_names {
        let _: () = conn.del(runtime_gateway_redis_key_store_key_hash(redis_key, &name))?;
    }
    let user_index = runtime_gateway_redis_key_store_scim_index(redis_key);
    let old_user_ids: Vec<String> = conn.smembers(&user_index)?;
    for id in old_user_ids {
        let _: () = conn.del(runtime_gateway_redis_key_store_scim_hash(redis_key, &id))?;
    }
    let _: () = conn.del(redis_key)?;
    let _: () = conn.del(&key_index)?;
    let _: () = conn.del(&user_index)?;

    for record in &store.keys {
        let hash_key = runtime_gateway_redis_key_store_key_hash(redis_key, &record.name);
        let _: () = conn.sadd(&key_index, &record.name)?;
        let _: () = redis::cmd("HSET")
            .arg(hash_key)
            .arg("name")
            .arg(&record.name)
            .arg("virtual_key_id")
            .arg(record.virtual_key_id.as_deref().unwrap_or_default())
            .arg("tenant_id")
            .arg(record.tenant_id.as_deref().unwrap_or_default())
            .arg("team_id")
            .arg(record.team_id.as_deref().unwrap_or_default())
            .arg("project_id")
            .arg(record.project_id.as_deref().unwrap_or_default())
            .arg("user_id")
            .arg(record.user_id.as_deref().unwrap_or_default())
            .arg("budget_id")
            .arg(record.budget_id.as_deref().unwrap_or_default())
            .arg("token_hash_base64")
            .arg(&record.token_hash_base64)
            .arg("allowed_models_json")
            .arg(serde_json::to_string(&record.allowed_models)?)
            .arg("budget_microusd")
            .arg(runtime_gateway_redis_optional_u64_field(
                record.budget_microusd,
            ))
            .arg("request_budget")
            .arg(runtime_gateway_redis_optional_u64_field(
                record.request_budget,
            ))
            .arg("rpm_limit")
            .arg(runtime_gateway_redis_optional_u64_field(record.rpm_limit))
            .arg("tpm_limit")
            .arg(runtime_gateway_redis_optional_u64_field(record.tpm_limit))
            .arg("disabled")
            .arg(if record.disabled.unwrap_or(false) {
                "1"
            } else {
                "0"
            })
            .arg("created_at_epoch")
            .arg(record.created_at_epoch.to_string())
            .arg("updated_at_epoch")
            .arg(record.updated_at_epoch.to_string())
            .query(conn)?;
    }

    for user in &store.scim_users {
        let hash_key = runtime_gateway_redis_key_store_scim_hash(redis_key, &user.id);
        let _: () = conn.sadd(&user_index, &user.id)?;
        let _: () = redis::cmd("HSET")
            .arg(hash_key)
            .arg("id")
            .arg(&user.id)
            .arg("user_name")
            .arg(&user.user_name)
            .arg("tenant_id")
            .arg(user.tenant_id.as_deref().unwrap_or_default())
            .arg("team_id")
            .arg(user.team_id.as_deref().unwrap_or_default())
            .arg("project_id")
            .arg(user.project_id.as_deref().unwrap_or_default())
            .arg("user_id")
            .arg(user.user_id.as_deref().unwrap_or_default())
            .arg("budget_id")
            .arg(user.budget_id.as_deref().unwrap_or_default())
            .arg("external_id")
            .arg(user.external_id.as_deref().unwrap_or_default())
            .arg("display_name")
            .arg(user.display_name.as_deref().unwrap_or_default())
            .arg("active")
            .arg(if user.active { "1" } else { "0" })
            .arg("role")
            .arg(user.role.as_deref().unwrap_or_default())
            .arg("allowed_key_prefixes_json")
            .arg(serde_json::to_string(&user.allowed_key_prefixes)?)
            .arg("created_at_epoch")
            .arg(user.created_at_epoch.to_string())
            .arg("updated_at_epoch")
            .arg(user.updated_at_epoch.to_string())
            .query(conn)?;
    }
    Ok(())
}

fn runtime_gateway_redis_key_store_key_index(redis_key: &str) -> String {
    format!("{redis_key}:keys")
}

fn runtime_gateway_redis_key_store_key_hash(redis_key: &str, name: &str) -> String {
    format!("{redis_key}:key:{name}")
}

fn runtime_gateway_redis_key_store_scim_index(redis_key: &str) -> String {
    format!("{redis_key}:scim_users")
}

fn runtime_gateway_redis_key_store_scim_hash(redis_key: &str, id: &str) -> String {
    format!("{redis_key}:scim_user:{id}")
}

fn runtime_gateway_redis_stored_key_from_hash(
    fields: &std::collections::BTreeMap<String, String>,
) -> Result<Option<RuntimeGatewayStoredVirtualKey>> {
    let Some(name) = runtime_gateway_redis_hash_exact_string(fields, "name") else {
        return Ok(None);
    };
    let Some(token_hash_base64) =
        runtime_gateway_redis_hash_exact_string(fields, "token_hash_base64")
    else {
        return Ok(None);
    };
    Ok(Some(RuntimeGatewayStoredVirtualKey {
        name,
        virtual_key_id: runtime_gateway_redis_hash_optional_exact_string(fields, "virtual_key_id"),
        tenant_id: runtime_gateway_redis_hash_optional_exact_string(fields, "tenant_id"),
        team_id: runtime_gateway_redis_hash_optional_exact_string(fields, "team_id"),
        project_id: runtime_gateway_redis_hash_optional_exact_string(fields, "project_id"),
        user_id: runtime_gateway_redis_hash_optional_exact_string(fields, "user_id"),
        budget_id: runtime_gateway_redis_hash_optional_exact_string(fields, "budget_id"),
        token_hash_base64,
        allowed_models: runtime_gateway_redis_hash_exact_json_vec(fields, "allowed_models_json")?,
        budget_microusd: runtime_gateway_redis_hash_optional_u64(fields, "budget_microusd")?,
        request_budget: runtime_gateway_redis_hash_optional_u64(fields, "request_budget")?,
        rpm_limit: runtime_gateway_redis_hash_optional_u64(fields, "rpm_limit")?,
        tpm_limit: runtime_gateway_redis_hash_optional_u64(fields, "tpm_limit")?,
        disabled: Some(runtime_gateway_redis_hash_bool(fields, "disabled")?),
        created_at_epoch: runtime_gateway_redis_hash_u64(fields, "created_at_epoch")?,
        updated_at_epoch: runtime_gateway_redis_hash_u64(fields, "updated_at_epoch")?,
    }))
}

fn runtime_gateway_redis_scim_user_from_hash(
    fields: &std::collections::BTreeMap<String, String>,
) -> Result<Option<RuntimeGatewayScimUser>> {
    let Some(id) = runtime_gateway_redis_hash_exact_string(fields, "id") else {
        return Ok(None);
    };
    let Some(user_name) = runtime_gateway_redis_hash_exact_string(fields, "user_name") else {
        return Ok(None);
    };
    Ok(Some(RuntimeGatewayScimUser {
        id,
        user_name,
        tenant_id: runtime_gateway_redis_hash_optional_exact_string(fields, "tenant_id"),
        team_id: runtime_gateway_redis_hash_optional_exact_string(fields, "team_id"),
        project_id: runtime_gateway_redis_hash_optional_exact_string(fields, "project_id"),
        user_id: runtime_gateway_redis_hash_optional_exact_string(fields, "user_id"),
        budget_id: runtime_gateway_redis_hash_optional_exact_string(fields, "budget_id"),
        external_id: runtime_gateway_redis_hash_optional_string(fields, "external_id"),
        display_name: runtime_gateway_redis_hash_optional_string(fields, "display_name"),
        active: runtime_gateway_redis_hash_bool(fields, "active")?,
        role: runtime_gateway_redis_hash_optional_exact_string(fields, "role"),
        allowed_key_prefixes: runtime_gateway_redis_hash_exact_json_vec(
            fields,
            "allowed_key_prefixes_json",
        )?,
        created_at_epoch: runtime_gateway_redis_hash_u64(fields, "created_at_epoch")?,
        updated_at_epoch: runtime_gateway_redis_hash_u64(fields, "updated_at_epoch")?,
    }))
}

fn runtime_gateway_redis_hash_exact_string(
    fields: &std::collections::BTreeMap<String, String>,
    name: &str,
) -> Option<String> {
    fields
        .get(name)
        .filter(|value| !value.is_empty())
        .filter(|value| !value.chars().any(char::is_whitespace))
        .cloned()
}

fn runtime_gateway_redis_hash_optional_exact_string(
    fields: &std::collections::BTreeMap<String, String>,
    name: &str,
) -> Option<String> {
    runtime_gateway_redis_hash_exact_string(fields, name)
}

fn runtime_gateway_redis_hash_string(
    fields: &std::collections::BTreeMap<String, String>,
    name: &str,
) -> Option<String> {
    fields
        .get(name)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn runtime_gateway_redis_hash_optional_string(
    fields: &std::collections::BTreeMap<String, String>,
    name: &str,
) -> Option<String> {
    runtime_gateway_redis_hash_string(fields, name)
}

fn runtime_gateway_redis_hash_u64(
    fields: &std::collections::BTreeMap<String, String>,
    name: &str,
) -> Result<u64> {
    let Some(value) = fields.get(name) else {
        return Ok(0);
    };
    if value.is_empty() {
        return Ok(0);
    }
    if value.chars().any(char::is_whitespace) {
        anyhow::bail!("gateway redis key-store field {name} must not contain whitespace");
    }
    value.parse::<u64>().with_context(|| {
        format!("gateway redis key-store field {name} must be an unsigned integer")
    })
}

fn runtime_gateway_redis_hash_optional_u64(
    fields: &std::collections::BTreeMap<String, String>,
    name: &str,
) -> Result<Option<u64>> {
    let Some(value) = fields.get(name) else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().any(char::is_whitespace) {
        anyhow::bail!("gateway redis key-store field {name} must not contain whitespace");
    }
    value.parse::<u64>().map(Some).with_context(|| {
        format!("gateway redis key-store field {name} must be an unsigned integer")
    })
}

fn runtime_gateway_redis_optional_u64_field(value: Option<u64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn runtime_gateway_redis_hash_bool(
    fields: &std::collections::BTreeMap<String, String>,
    name: &str,
) -> Result<bool> {
    let Some(value) = fields.get(name) else {
        return Ok(false);
    };
    if value.is_empty() {
        return Ok(false);
    }
    match value.as_str() {
        "0" => Ok(false),
        "1" => Ok(true),
        _ if value.chars().any(char::is_whitespace) => {
            anyhow::bail!("gateway redis key-store field {name} must not contain whitespace")
        }
        _ => anyhow::bail!("gateway redis key-store field {name} must be 0 or 1"),
    }
}

fn runtime_gateway_redis_hash_exact_json_vec(
    fields: &std::collections::BTreeMap<String, String>,
    name: &str,
) -> Result<Vec<String>> {
    let Some(value) = fields.get(name) else {
        return Ok(Vec::new());
    };
    if value.is_empty() {
        return Ok(Vec::new());
    }
    runtime_gateway_exact_json_vec_for_field(
        value,
        &format!("gateway redis key-store field {name}"),
    )
}

fn runtime_gateway_exact_json_vec_for_field(value: &str, field: &str) -> Result<Vec<String>> {
    let values = serde_json::from_str::<Vec<String>>(value)
        .with_context(|| format!("{field} must be a JSON array of strings"))?;
    for value in &values {
        if value.is_empty() {
            anyhow::bail!("{field} must not contain empty entries");
        }
        if value.chars().any(char::is_whitespace) {
            anyhow::bail!("{field} entries must not contain whitespace");
        }
    }
    Ok(values)
}

pub(super) fn runtime_gateway_sqlite_save_key_store_in_tx(
    tx: &rusqlite::Transaction<'_>,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> Result<()> {
    runtime_gateway_sqlite_upsert_tenants_in_tx(tx, store)?;
    tx.execute("DELETE FROM prodex_gateway_virtual_keys", [])?;
    for record in &store.keys {
        let allowed_models_json = serde_json::to_string(&record.allowed_models)?;
        tx.execute(
            r#"
            INSERT INTO prodex_gateway_virtual_keys (
                name, tenant_id, team_id, project_id, user_id, budget_id,
                token_hash_base64, virtual_key_id, allowed_models_json, budget_microusd,
                request_budget, rpm_limit, tpm_limit, disabled, created_at_epoch, updated_at_epoch
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            "#,
            params![
                record.name,
                record.tenant_id,
                record.team_id,
                record.project_id,
                record.user_id,
                record.budget_id,
                record.token_hash_base64,
                record.virtual_key_id,
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
    runtime_gateway_postgres_upsert_tenants_in_tx(tx, store)?;
    tx.execute("DELETE FROM prodex_gateway_virtual_keys", &[])?;
    for record in &store.keys {
        let allowed_models_json = serde_json::to_string(&record.allowed_models)?;
        let disabled = record.disabled.unwrap_or(false);
        tx.execute(
            r#"
            INSERT INTO prodex_gateway_virtual_keys (
                name, tenant_id, team_id, project_id, user_id, budget_id,
                token_hash_base64, virtual_key_id, allowed_models_json, budget_microusd,
                request_budget, rpm_limit, tpm_limit, disabled, created_at_epoch, updated_at_epoch
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#,
            &[
                &record.name,
                &record.tenant_id,
                &record.team_id,
                &record.project_id,
                &record.user_id,
                &record.budget_id,
                &record.token_hash_base64,
                &record.virtual_key_id,
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
    use super::super::local_rewrite_gateway_backend_connection::{
        runtime_gateway_postgres_migrate_compatibility_state,
        runtime_gateway_sqlite_create_current_schema_for_tests,
    };
    use super::*;
    use postgres::NoTls;
    use prodex_storage_postgres::{PostgresRuntimeMode, plan_postgres_migrations};
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
        let mut client = postgres::Client::connect(url, NoTls).expect("postgres should connect");
        let has_enterprise_schema: bool = client
            .query_one(
                "SELECT EXISTS(
                    SELECT 1
                    FROM information_schema.tables
                    WHERE table_schema = current_schema()
                      AND table_name = 'prodex_tenants'
                )",
                &[],
            )
            .expect("postgres enterprise schema probe should load")
            .get(0);
        if !has_enterprise_schema {
            let plan = plan_postgres_migrations(PostgresRuntimeMode::ExternalMigrator)
                .expect("postgres schema plan should build");
            for migration in &plan.migrations {
                client
                    .batch_execute(migration.sql)
                    .expect("postgres enterprise migration should apply");
            }
        }
        let tls = prodex_storage_postgres_runtime::PostgresTlsConfig::explicit_disable();
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
                .contains(
                    "gateway redis key-store field padded entries must not contain whitespace"
                )
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
                .contains("Conversion error from type Integer at index: 12"),
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
        assert!(loaded.keys[0].virtual_key_id.is_some());
        assert_eq!(loaded.scim_users[0].user_name, "user@example.com");

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
                budget_id: None,
                allowed_key_prefixes: vec!["alpha".to_string()],
                created_at_epoch: 3,
                updated_at_epoch: 4,
            }],
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
        assert!(
            loaded
                .scim_users
                .iter()
                .any(|user| user.user_name == store.scim_users[0].user_name)
        );
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
        assert_eq!(runtime_gateway_redis_optional_u64_field(Some(0)), "0");
        assert_eq!(runtime_gateway_redis_optional_u64_field(None), "");
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
            err.to_string().contains(
                "gateway redis key-store field budget_microusd must be an unsigned integer"
            ),
            "{err:?}"
        );

        fields.insert("budget_microusd".to_string(), "100".to_string());
        fields.insert("updated_at_epoch".to_string(), " 2 ".to_string());
        let err = runtime_gateway_redis_stored_key_from_hash(&fields).unwrap_err();
        assert!(
            err.to_string().contains(
                "gateway redis key-store field updated_at_epoch must not contain whitespace"
            ),
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
}
