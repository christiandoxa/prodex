use std::fs::OpenOptions;
use std::path::Path;

use fs2::FileExt;
use prodex_domain::{
    AuditDigest, AuditEnvelope, AuditEvent, IdempotencyEntry, IdempotencyRecord,
    IdempotentOperation, compute_audit_chain_digest,
};
use redis::Commands;
use rusqlite::{OptionalExtension, TransactionBehavior, params};

use super::super::local_rewrite::{
    RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY, RUNTIME_GATEWAY_REDIS_KEY_STORE_LOCK,
    RuntimeLocalRewriteProxyShared, runtime_gateway_generate_virtual_key_token,
};
use super::super::local_rewrite_gateway_admin_response::RuntimeGatewayAdminError;
use super::super::local_rewrite_gateway_backend_connection::{
    runtime_gateway_postgres_open, runtime_gateway_redis_with_lock_token,
    runtime_gateway_sqlite_open,
};
use super::super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::super::local_rewrite_gateway_key_store_backend::{
    runtime_gateway_postgres_load_key_store_from_client,
    runtime_gateway_postgres_save_key_store_in_tx, runtime_gateway_redis_key_store_key_hash,
    runtime_gateway_redis_key_store_key_index, runtime_gateway_redis_key_store_scim_hash,
    runtime_gateway_redis_key_store_scim_index, runtime_gateway_redis_load_key_store_from_conn,
    runtime_gateway_sqlite_load_key_store_from_conn, runtime_gateway_sqlite_save_key_store_in_tx,
};
use super::super::local_rewrite_gateway_store_file::runtime_gateway_virtual_key_store_file_save;
use super::super::local_rewrite_gateway_store_types::{
    RuntimeGatewayAdminIdempotencyRecord, RuntimeGatewayVirtualKeyStoreFile,
    runtime_gateway_virtual_key_store_version,
};
use super::super::*;
use super::{
    runtime_gateway_apply_admin_virtual_key_store,
    runtime_gateway_virtual_key_store_load_strict_file,
};

mod postgres;
use postgres::runtime_gateway_atomic_postgres;

#[cfg(test)]
mod tests;

pub(in crate::runtime_launch::proxy_startup) struct RuntimeGatewayAdminAtomicWrite {
    pub(in crate::runtime_launch::proxy_startup) operation: IdempotentOperation,
    pub(in crate::runtime_launch::proxy_startup) started_at_unix_ms: u64,
    pub(in crate::runtime_launch::proxy_startup) completed_at_unix_ms: u64,
    pub(in crate::runtime_launch::proxy_startup) audit_event: AuditEvent,
}

impl std::fmt::Debug for RuntimeGatewayAdminAtomicWrite {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeGatewayAdminAtomicWrite")
            .field("operation", &"<redacted>")
            .field("timestamps", &"<redacted>")
            .field("audit_event", &"<redacted>")
            .finish()
    }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_mutate_admin_key_store_atomic<F>(
    shared: &RuntimeLocalRewriteProxyShared,
    write: RuntimeGatewayAdminAtomicWrite,
    mutation: F,
) -> Result<(), tiny_http::ResponseBox>
where
    F: FnOnce(&mut RuntimeGatewayVirtualKeyStoreFile) -> Result<(), RuntimeGatewayAdminError>,
{
    if !runtime_gateway_atomic_write_is_valid(&write) {
        return Err(runtime_gateway_atomic_error(
            500,
            "gateway_admin_atomic_plan_invalid",
        ));
    }
    match &shared.gateway_state_store {
        RuntimeGatewayStateStore::File { key_store_path, .. } => {
            runtime_gateway_atomic_file(shared, key_store_path, write, mutation)
        }
        RuntimeGatewayStateStore::Sqlite { path } => {
            runtime_gateway_atomic_sqlite(shared, path, write, mutation)
        }
        RuntimeGatewayStateStore::Postgres { url, tls, .. } => {
            runtime_gateway_atomic_postgres(shared, url, tls, write, mutation)
        }
        RuntimeGatewayStateStore::Redis { url, .. } => {
            runtime_gateway_atomic_redis(shared, url, write, mutation)
        }
    }
}

fn runtime_gateway_atomic_write_is_valid(write: &RuntimeGatewayAdminAtomicWrite) -> bool {
    write.started_at_unix_ms > 0
        && write.completed_at_unix_ms >= write.started_at_unix_ms
        && write.operation.tenant_id == write.audit_event.tenant_id
        && write.audit_event.resource.tenant_id == Some(write.operation.tenant_id)
}

fn runtime_gateway_atomic_file<F>(
    shared: &RuntimeLocalRewriteProxyShared,
    path: &Path,
    write: RuntimeGatewayAdminAtomicWrite,
    mutation: F,
) -> Result<(), tiny_http::ResponseBox>
where
    F: FnOnce(&mut RuntimeGatewayVirtualKeyStoreFile) -> Result<(), RuntimeGatewayAdminError>,
{
    let lock_path = path.with_extension("json.lock");
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|_| runtime_gateway_atomic_error(500, "gateway_key_store_lock_failed"))?;
    }
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|_| runtime_gateway_atomic_error(500, "gateway_key_store_lock_failed"))?;
    lock.lock_exclusive()
        .map_err(|_| runtime_gateway_atomic_error(500, "gateway_key_store_lock_failed"))?;
    let mut store = runtime_gateway_virtual_key_store_load_strict_file(path)
        .map_err(RuntimeGatewayAdminError::into_response)?;
    runtime_gateway_file_idempotency_absent(&store, &write.operation)?;
    mutation(&mut store).map_err(RuntimeGatewayAdminError::into_response)?;
    runtime_gateway_record_file_atomic_write(&mut store, write);
    runtime_gateway_virtual_key_store_file_save(path, &store)
        .map_err(|_| runtime_gateway_atomic_error(500, "gateway_key_store_save_failed"))?;
    runtime_gateway_apply_admin_virtual_key_store(shared, &store);
    Ok(())
}

fn runtime_gateway_file_idempotency_absent(
    store: &RuntimeGatewayVirtualKeyStoreFile,
    operation: &IdempotentOperation,
) -> Result<(), tiny_http::ResponseBox> {
    let existing = store.admin_idempotency.iter().find(|record| {
        let candidate = record.entry.operation();
        candidate.tenant_id == operation.tenant_id && candidate.key == operation.key
    });
    if existing.is_some() {
        return Err(runtime_gateway_atomic_error(
            409,
            "duplicate_idempotency_key",
        ));
    }
    Ok(())
}

fn runtime_gateway_record_file_atomic_write(
    store: &mut RuntimeGatewayVirtualKeyStoreFile,
    write: RuntimeGatewayAdminAtomicWrite,
) {
    store.version = runtime_gateway_virtual_key_store_version();
    store
        .admin_idempotency
        .push(RuntimeGatewayAdminIdempotencyRecord {
            entry: IdempotencyEntry::completed(IdempotencyRecord {
                operation: write.operation,
                response: (),
            }),
            completed_at_unix_ms: write.completed_at_unix_ms,
        });
    let previous = store
        .admin_audit
        .iter()
        .rev()
        .find(|item| item.event.tenant_id == write.audit_event.tenant_id)
        .map(|item| item.event_digest.clone());
    let digest = compute_audit_chain_digest(previous.as_ref(), &write.audit_event);
    store
        .admin_audit
        .push(AuditEnvelope::new(write.audit_event, previous, digest));
    store.bound_admin_history();
    store.sort_for_rendering();
}

fn runtime_gateway_atomic_sqlite<F>(
    shared: &RuntimeLocalRewriteProxyShared,
    path: &Path,
    write: RuntimeGatewayAdminAtomicWrite,
    mutation: F,
) -> Result<(), tiny_http::ResponseBox>
where
    F: FnOnce(&mut RuntimeGatewayVirtualKeyStoreFile) -> Result<(), RuntimeGatewayAdminError>,
{
    let mut conn = runtime_gateway_sqlite_open(path).map_err(|_| {
        runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable")
    })?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| {
            runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable")
        })?;
    runtime_gateway_sqlite_idempotency_absent(&tx, &write.operation)?;
    let audit = runtime_gateway_sqlite_audit_envelope(&tx, &write.audit_event)?;
    let mut store = runtime_gateway_sqlite_load_key_store_from_conn(&tx).map_err(|_| {
        runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable")
    })?;
    mutation(&mut store).map_err(RuntimeGatewayAdminError::into_response)?;
    store.version = runtime_gateway_virtual_key_store_version();
    store.sort_for_rendering();
    runtime_gateway_sqlite_save_key_store_in_tx(&tx, &store)
        .and_then(|_| runtime_gateway_sqlite_write_metadata(&tx, &write, &audit))
        .map_err(|_| {
            runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable")
        })?;
    tx.commit().map_err(|_| {
        runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable")
    })?;
    runtime_gateway_apply_admin_virtual_key_store(shared, &store);
    Ok(())
}

fn runtime_gateway_sqlite_idempotency_absent(
    tx: &rusqlite::Transaction<'_>,
    operation: &IdempotentOperation,
) -> Result<(), tiny_http::ResponseBox> {
    let tenant = operation.tenant_id.to_string();
    let existing: Option<(String, String)> = tx
        .query_row(
            "SELECT request_fingerprint, entry_status FROM prodex_idempotency_records WHERE tenant_id = ?1 AND idempotency_key = ?2",
            params![tenant, operation.key.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable"))?;
    if existing.is_some() {
        return Err(runtime_gateway_atomic_error(
            409,
            "duplicate_idempotency_key",
        ));
    }
    Ok(())
}

fn runtime_gateway_sqlite_audit_envelope(
    tx: &rusqlite::Transaction<'_>,
    event: &AuditEvent,
) -> Result<AuditEnvelope, tiny_http::ResponseBox> {
    let mut statement = tx
        .prepare(
            "SELECT head.event_digest FROM prodex_audit_log AS head
             WHERE head.tenant_id = ?1
               AND NOT EXISTS (
                   SELECT 1 FROM prodex_audit_log AS successor
                   WHERE successor.tenant_id = head.tenant_id
                     AND successor.previous_digest = head.event_digest
               )
             LIMIT 2",
        )
        .map_err(|_| {
            runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable")
        })?;
    let heads = statement
        .query_map([event.tenant_id.to_string()], |row| row.get::<_, String>(0))
        .and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>())
        .map_err(|_| {
            runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable")
        })?;
    if heads.len() > 1 {
        return Err(runtime_gateway_atomic_error(
            503,
            "gateway_admin_audit_chain_invalid",
        ));
    }
    let previous = heads
        .into_iter()
        .next()
        .map(AuditDigest::new)
        .transpose()
        .map_err(|_| {
            runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable")
        })?;
    let digest = compute_audit_chain_digest(previous.as_ref(), event);
    Ok(AuditEnvelope::new(event.clone(), previous, digest))
}

fn runtime_gateway_sqlite_write_metadata(
    tx: &rusqlite::Transaction<'_>,
    write: &RuntimeGatewayAdminAtomicWrite,
    audit: &AuditEnvelope,
) -> anyhow::Result<()> {
    let operation = &write.operation;
    let tenant = operation.tenant_id.to_string();
    tx.execute(
        "INSERT INTO prodex_idempotency_records (tenant_id, idempotency_key, request_fingerprint, entry_status, started_at_unix_ms) VALUES (?1, ?2, ?3, 'pending', ?4)",
        params![tenant, operation.key.as_str(), operation.request_fingerprint, i64::try_from(write.started_at_unix_ms)?],
    )?;
    let updated = tx.execute(
        "UPDATE prodex_idempotency_records SET entry_status = 'completed', completed_at_unix_ms = ?4, response_body = NULL WHERE tenant_id = ?1 AND idempotency_key = ?2 AND request_fingerprint = ?3 AND entry_status = 'pending'",
        params![tenant, operation.key.as_str(), operation.request_fingerprint, i64::try_from(write.completed_at_unix_ms)?],
    )?;
    if updated != 1 {
        anyhow::bail!("admin idempotency completion marker was not applied");
    }
    tx.execute(
        "INSERT INTO prodex_audit_log (tenant_id, audit_event_id, previous_digest, event_digest, occurred_at_unix_ms, principal_id, action, resource_kind, resource_id, outcome, reason_code) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            tenant,
            audit.event.id.to_string(),
            audit.previous_digest.as_ref().map(|value| value.as_str()),
            audit.event_digest.as_str(),
            i64::try_from(audit.event.occurred_at_unix_ms)?,
            audit.event.principal_id.to_string(),
            audit.event.action.as_str(),
            audit.event.resource.kind,
            audit.event.resource.id,
            audit.event.outcome.as_str(),
            audit.event.reason_code,
        ],
    )?;
    Ok(())
}

fn runtime_gateway_atomic_redis<F>(
    shared: &RuntimeLocalRewriteProxyShared,
    url: &str,
    write: RuntimeGatewayAdminAtomicWrite,
    mutation: F,
) -> Result<(), tiny_http::ResponseBox>
where
    F: FnOnce(&mut RuntimeGatewayVirtualKeyStoreFile) -> Result<(), RuntimeGatewayAdminError>,
{
    let result = runtime_gateway_redis_with_lock_token(
        url,
        RUNTIME_GATEWAY_REDIS_KEY_STORE_LOCK,
        runtime_gateway_generate_virtual_key_token,
        |conn, token| {
            runtime_gateway_redis_atomic_write(conn, token, write, mutation)
                .map_err(anyhow::Error::msg)
        },
    );
    match result {
        Ok(Ok(store)) => {
            runtime_gateway_apply_admin_virtual_key_store(shared, &store);
            Ok(())
        }
        Ok(Err(error)) => Err(error.into_response()),
        Err(_) => Err(runtime_gateway_atomic_error(
            503,
            "gateway_admin_atomic_storage_conflict",
        )),
    }
}

fn runtime_gateway_redis_atomic_write<F>(
    conn: &mut redis::Connection,
    token: &str,
    write: RuntimeGatewayAdminAtomicWrite,
    mutation: F,
) -> Result<Result<RuntimeGatewayVirtualKeyStoreFile, RuntimeGatewayAdminError>, String>
where
    F: FnOnce(&mut RuntimeGatewayVirtualKeyStoreFile) -> Result<(), RuntimeGatewayAdminError>,
{
    let key_index = runtime_gateway_redis_key_store_key_index(RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY);
    let user_index =
        runtime_gateway_redis_key_store_scim_index(RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY);
    redis::cmd("WATCH")
        .arg(RUNTIME_GATEWAY_REDIS_KEY_STORE_LOCK)
        .arg(RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY)
        .arg(&key_index)
        .arg(&user_index)
        .query::<()>(conn)
        .map_err(|_| "redis watch failed".to_string())?;
    let observed_token: Option<String> = conn
        .get(RUNTIME_GATEWAY_REDIS_KEY_STORE_LOCK)
        .map_err(|_| "redis lock read failed".to_string())?;
    if observed_token.as_deref() != Some(token) {
        return Err("redis lock ownership changed".to_string());
    }
    let mut store =
        runtime_gateway_redis_load_key_store_from_conn(conn, RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY)
            .map_err(|_| "redis state load failed".to_string())?;
    if store.admin_idempotency.iter().any(|record| {
        let candidate = record.entry.operation();
        candidate.tenant_id == write.operation.tenant_id && candidate.key == write.operation.key
    }) {
        let _ = redis::cmd("UNWATCH").query::<()>(conn);
        return Ok(Err(RuntimeGatewayAdminError::new(
            409,
            "duplicate_idempotency_key",
            "admin mutation idempotency key conflicts with an existing request",
        )));
    }
    let old_key_hashes = store
        .keys
        .iter()
        .map(|record| {
            runtime_gateway_redis_key_store_key_hash(
                RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY,
                &record.name,
            )
        })
        .collect::<Vec<_>>();
    let old_user_hashes = store
        .scim_users
        .iter()
        .map(|record| {
            runtime_gateway_redis_key_store_scim_hash(
                RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY,
                &record.id,
            )
        })
        .collect::<Vec<_>>();
    if let Err(error) = mutation(&mut store) {
        let _ = redis::cmd("UNWATCH").query::<()>(conn);
        return Ok(Err(error));
    }
    runtime_gateway_record_file_atomic_write(&mut store, write);
    let payload =
        serde_json::to_string(&store).map_err(|_| "redis state encode failed".to_string())?;
    let mut pipe = redis::pipe();
    pipe.atomic()
        .del(RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY)
        .ignore()
        .del(&key_index)
        .ignore()
        .del(&user_index)
        .ignore();
    for key in old_key_hashes.into_iter().chain(old_user_hashes) {
        pipe.del(key).ignore();
    }
    pipe.set(RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY, payload)
        .ignore();
    let committed: Option<()> = pipe
        .query(conn)
        .map_err(|_| "redis exec failed".to_string())?;
    if committed.is_none() {
        return Err("redis exec conflict".to_string());
    }
    Ok(Ok(store))
}

fn runtime_gateway_atomic_error(status: u16, code: &'static str) -> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(
        status,
        code,
        "gateway admin mutation could not be committed atomically",
    )
}
