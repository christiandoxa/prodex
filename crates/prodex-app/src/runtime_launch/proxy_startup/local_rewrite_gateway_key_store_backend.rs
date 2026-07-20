use std::path::Path;

use anyhow::{Context, Result};
use postgres::GenericClient;
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

#[path = "local_rewrite_gateway_key_store_tenants.rs"]
mod tenants;
use tenants::{
    runtime_gateway_postgres_upsert_tenants_in_tx, runtime_gateway_sqlite_upsert_tenants_in_tx,
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
        admin_idempotency: Vec::new(),
        admin_audit: Vec::new(),
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
        admin_idempotency: Vec::new(),
        admin_audit: Vec::new(),
    })
}

fn runtime_gateway_sqlite_load_scim_users_from_conn(
    conn: &Connection,
) -> Result<Vec<RuntimeGatewayScimUser>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, user_name, tenant_id, team_id, project_id, user_id, budget_id,
               external_id, display_name, active, role,
               allowed_key_prefixes_json, group_ids_json, department_id,
               created_at_epoch, updated_at_epoch
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
        let group_ids_json: String = row.get(12)?;
        let group_ids = runtime_gateway_exact_json_vec_for_field(
            &group_ids_json,
            "gateway key-store field group_ids_json",
        )
        .map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(
                12,
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
            group_ids,
            department_id: row.get(13)?,
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
               allowed_key_prefixes_json, group_ids_json, department_id,
               created_at_epoch, updated_at_epoch
        FROM prodex_gateway_scim_users
        ORDER BY lower(user_name), user_name
        "#,
        &[],
    )?;
    let mut users = Vec::new();
    for row in rows {
        let prefixes_json: String = row.get(11);
        let group_ids_json: String = row.get(12);
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
            group_ids: runtime_gateway_exact_json_vec_for_field(
                &group_ids_json,
                "gateway key-store field group_ids_json",
            )?,
            department_id: row.get(13),
            created_at_epoch: runtime_gateway_sql_key_store_i64_to_u64(
                row.get(14),
                "created_at_epoch",
            )?,
            updated_at_epoch: runtime_gateway_sql_key_store_i64_to_u64(
                row.get(15),
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
        let mut store = serde_json::from_str::<RuntimeGatewayVirtualKeyStoreFile>(&payload)
            .context("failed to parse legacy gateway redis virtual key store")?;
        store.bound_admin_history();
        return Ok(store);
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
        admin_idempotency: Vec::new(),
        admin_audit: Vec::new(),
    })
}

pub(super) fn runtime_gateway_redis_key_store_key_index(redis_key: &str) -> String {
    format!("{redis_key}:keys")
}

pub(super) fn runtime_gateway_redis_key_store_key_hash(redis_key: &str, name: &str) -> String {
    format!("{redis_key}:key:{name}")
}

pub(super) fn runtime_gateway_redis_key_store_scim_index(redis_key: &str) -> String {
    format!("{redis_key}:scim_users")
}

pub(super) fn runtime_gateway_redis_key_store_scim_hash(redis_key: &str, id: &str) -> String {
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
        group_ids: runtime_gateway_redis_hash_exact_json_vec(fields, "group_ids_json")?,
        department_id: runtime_gateway_redis_hash_optional_exact_string(fields, "department_id"),
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
        let group_ids_json = serde_json::to_string(&user.group_ids)?;
        tx.execute(
            r#"
            INSERT INTO prodex_gateway_scim_users (
                id, user_name, tenant_id, team_id, project_id, user_id, budget_id,
                external_id, display_name, active, role,
                allowed_key_prefixes_json, group_ids_json, department_id,
                created_at_epoch, updated_at_epoch
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
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
                group_ids_json,
                user.department_id,
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
        let group_ids_json = serde_json::to_string(&user.group_ids)?;
        tx.execute(
            r#"
            INSERT INTO prodex_gateway_scim_users (
                id, user_name, tenant_id, team_id, project_id, user_id, budget_id,
                external_id, display_name, active, role,
                allowed_key_prefixes_json, group_ids_json, department_id,
                created_at_epoch, updated_at_epoch
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
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
                &group_ids_json,
                &user.department_id,
                &runtime_gateway_sqlite_u64_to_i64(user.created_at_epoch),
                &runtime_gateway_sqlite_u64_to_i64(user.updated_at_epoch),
            ],
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "local_rewrite_gateway_key_store_backend_tests.rs"]
mod tests;
