use super::{RuntimeGatewayVirtualKeyStoreFile, runtime_gateway_sqlite_u64_to_i64};
use anyhow::Result;
use prodex_domain::TenantId;
use rusqlite::params;
use std::str::FromStr;

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

pub(super) fn runtime_gateway_sqlite_upsert_tenants_in_tx(
    tx: &rusqlite::Transaction<'_>,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> Result<()> {
    for tenant_id in runtime_gateway_typed_tenant_ids(store) {
        let tenant_id = tenant_id.to_string();
        let now = runtime_gateway_tenant_updated_at_unix_ms(store);
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

pub(super) fn runtime_gateway_postgres_upsert_tenants_in_tx(
    tx: &mut postgres::Transaction<'_>,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> Result<()> {
    for tenant_id in runtime_gateway_typed_tenant_ids(store) {
        let tenant_uuid = tenant_id.as_uuid();
        let tenant_display_name = tenant_id.to_string();
        let now = runtime_gateway_tenant_updated_at_unix_ms(store);
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

fn runtime_gateway_tenant_updated_at_unix_ms(store: &RuntimeGatewayVirtualKeyStoreFile) -> i64 {
    runtime_gateway_sqlite_u64_to_i64(
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
    )
}
