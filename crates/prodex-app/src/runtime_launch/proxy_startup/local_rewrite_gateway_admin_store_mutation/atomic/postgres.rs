use super::*;

pub(super) fn runtime_gateway_atomic_postgres<F>(
    shared: &RuntimeLocalRewriteProxyShared,
    url: &str,
    tls: &prodex_storage_postgres_runtime::PostgresTlsConfig,
    write: RuntimeGatewayAdminAtomicWrite,
    mutation: F,
) -> Result<(), tiny_http::ResponseBox>
where
    F: FnOnce(&mut RuntimeGatewayVirtualKeyStoreFile) -> Result<(), RuntimeGatewayAdminError>,
{
    let mut client = runtime_gateway_postgres_open(url, tls).map_err(|_| {
        runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable")
    })?;
    let mut tx = client.transaction().map_err(|_| {
        runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable")
    })?;
    let tenant = write.operation.tenant_id.as_uuid();
    tx.query_one(
        "SELECT set_config('prodex.tenant_id', $1, true)",
        &[&tenant.to_string()],
    )
    .and_then(|_| tx.batch_execute("LOCK TABLE prodex_gateway_virtual_keys, prodex_gateway_scim_users, prodex_idempotency_records, prodex_audit_log IN EXCLUSIVE MODE"))
    .map_err(|_| runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable"))?;
    if tx
        .query_opt(
            "SELECT request_fingerprint, entry_status FROM prodex_idempotency_records WHERE tenant_id = $1 AND idempotency_key = $2",
            &[&tenant, &write.operation.key.as_str()],
        )
        .map_err(|_| runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable"))?
        .is_some()
    {
        return Err(runtime_gateway_atomic_error(409, "duplicate_idempotency_key"));
    }
    let heads = tx
        .query(
            "SELECT head.event_digest FROM prodex_audit_log AS head
             WHERE head.tenant_id = $1
               AND NOT EXISTS (
                   SELECT 1 FROM prodex_audit_log AS successor
                   WHERE successor.tenant_id = head.tenant_id
                     AND successor.previous_digest = head.event_digest
               )
             LIMIT 2",
            &[&tenant],
        )
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
        .first()
        .map(|row| row.get::<_, String>(0))
        .map(AuditDigest::new)
        .transpose()
        .map_err(|_| {
            runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable")
        })?;
    let audit = AuditEnvelope::new(
        write.audit_event.clone(),
        previous.clone(),
        compute_audit_chain_digest(previous.as_ref(), &write.audit_event),
    );
    let mut store = runtime_gateway_postgres_load_key_store_from_client(&mut tx).map_err(|_| {
        runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable")
    })?;
    mutation(&mut store).map_err(RuntimeGatewayAdminError::into_response)?;
    store.sort_for_rendering();
    runtime_gateway_postgres_save_key_store_in_tx(&mut tx, &store)
        .and_then(|_| runtime_gateway_postgres_write_metadata(&mut tx, &write, &audit))
        .map_err(|_| {
            runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable")
        })?;
    tx.commit().map_err(|_| {
        runtime_gateway_atomic_error(503, "gateway_admin_atomic_storage_unavailable")
    })?;
    runtime_gateway_apply_admin_virtual_key_store(shared, &store);
    Ok(())
}

fn runtime_gateway_postgres_write_metadata(
    tx: &mut ::postgres::Transaction<'_>,
    write: &RuntimeGatewayAdminAtomicWrite,
    audit: &AuditEnvelope,
) -> anyhow::Result<()> {
    let tenant = write.operation.tenant_id.as_uuid();
    let started_at = i64::try_from(write.started_at_unix_ms)?;
    let completed_at = i64::try_from(write.completed_at_unix_ms)?;
    let idempotency_params: &[&(dyn ::postgres::types::ToSql + Sync)] = &[
        &tenant,
        &write.operation.key.as_str(),
        &write.operation.request_fingerprint,
        &started_at,
        &completed_at,
    ];
    tx.execute(
        "INSERT INTO prodex_idempotency_records (tenant_id, idempotency_key, request_fingerprint, entry_status, started_at_unix_ms) VALUES ($1, $2, $3, 'pending', $4)",
        &idempotency_params[..4],
    )?;
    let updated = tx.execute(
        "UPDATE prodex_idempotency_records SET entry_status = 'completed', completed_at_unix_ms = $5, response_body = NULL WHERE tenant_id = $1 AND idempotency_key = $2 AND request_fingerprint = $3 AND entry_status = 'pending'",
        idempotency_params,
    )?;
    if updated != 1 {
        anyhow::bail!("admin idempotency completion marker was not applied");
    }
    let occurred_at = i64::try_from(audit.event.occurred_at_unix_ms)?;
    let audit_event_id = audit.event.id.as_uuid();
    let principal_id = audit.event.principal_id.as_uuid();
    let previous_digest = audit.previous_digest.as_ref().map(|value| value.as_str());
    let audit_params: &[&(dyn ::postgres::types::ToSql + Sync)] = &[
        &tenant,
        &audit_event_id,
        &previous_digest,
        &audit.event_digest.as_str(),
        &occurred_at,
        &principal_id,
        &audit.event.action.as_str(),
        &audit.event.resource.kind,
        &audit.event.resource.id,
        &audit.event.outcome.as_str(),
        &audit.event.reason_code,
    ];
    tx.execute(
        "INSERT INTO prodex_audit_log (tenant_id, audit_event_id, previous_digest, event_digest, occurred_at_unix_ms, principal_id, action, resource_kind, resource_id, outcome, reason_code) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        audit_params,
    )?;
    Ok(())
}
