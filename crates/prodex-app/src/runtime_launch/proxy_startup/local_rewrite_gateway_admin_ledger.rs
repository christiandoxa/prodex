use super::local_rewrite::{RuntimeLocalRewriteProxyShared, runtime_gateway_billing_ledger_load};
use super::local_rewrite_gateway_admin_auth::{
    RuntimeGatewayAdminAuth, runtime_gateway_admin_auth_is_unscoped,
    runtime_gateway_admin_auth_matches_entry,
};
use super::local_rewrite_gateway_admin_response::{
    runtime_gateway_admin_csv_response, runtime_gateway_admin_json_response,
};
use super::local_rewrite_gateway_billing_csv::{
    runtime_gateway_billing_ledger_csv, runtime_gateway_billing_summary_csv,
};
use super::local_rewrite_gateway_billing_summary::{
    RuntimeGatewayBillingSummaryKeyDimensions, RuntimeGatewayBillingSummaryRecord,
    runtime_gateway_billing_summary_payload as runtime_gateway_billing_summary_payload_from_records,
};
use super::local_rewrite_gateway_ledger_types::RuntimeGatewayBillingLedgerEntry;
use super::local_rewrite_gateway_store_types::RuntimeGatewayVirtualKeyEntry;
use super::*;

const RUNTIME_GATEWAY_ADMIN_LEDGER_EXPORT_LIMIT: usize = 100_000;

pub(super) fn runtime_gateway_admin_ledger_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    match runtime_gateway_billing_ledger_load(&shared.gateway_state_store, 1000) {
        Ok(records) => {
            let records = runtime_gateway_admin_filter_ledger_records(records, shared, admin_auth);
            runtime_gateway_admin_json_response(
                200,
                serde_json::json!({
                    "object": "gateway.billing_ledger",
                    "state_backend": shared.gateway_state_store.label(),
                    "ledger_path": shared.gateway_state_store.ledger_path().display().to_string(),
                    "limit": 1000,
                    "records": records,
                }),
            )
        }
        Err(_err) => build_runtime_proxy_json_error_response(
            500,
            "gateway_billing_ledger_load_failed",
            "gateway billing ledger could not be loaded",
        ),
    }
}

pub(super) fn runtime_gateway_admin_ledger_csv_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    match runtime_gateway_billing_ledger_load(
        &shared.gateway_state_store,
        RUNTIME_GATEWAY_ADMIN_LEDGER_EXPORT_LIMIT,
    ) {
        Ok(records) => {
            let records = runtime_gateway_admin_filter_ledger_records(records, shared, admin_auth);
            runtime_gateway_admin_csv_response(runtime_gateway_billing_ledger_csv(&records))
        }
        Err(_err) => build_runtime_proxy_json_error_response(
            500,
            "gateway_billing_ledger_load_failed",
            "gateway billing ledger could not be loaded",
        ),
    }
}

pub(super) fn runtime_gateway_admin_ledger_summary_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    match runtime_gateway_billing_ledger_load(
        &shared.gateway_state_store,
        RUNTIME_GATEWAY_ADMIN_LEDGER_EXPORT_LIMIT,
    ) {
        Ok(records) => {
            let records = runtime_gateway_admin_filter_ledger_records(records, shared, admin_auth);
            runtime_gateway_admin_json_response(
                200,
                runtime_gateway_billing_summary_payload(shared, &records),
            )
        }
        Err(_err) => build_runtime_proxy_json_error_response(
            500,
            "gateway_billing_summary_load_failed",
            "gateway billing summary could not be loaded",
        ),
    }
}

pub(super) fn runtime_gateway_admin_ledger_summary_csv_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    match runtime_gateway_billing_ledger_load(
        &shared.gateway_state_store,
        RUNTIME_GATEWAY_ADMIN_LEDGER_EXPORT_LIMIT,
    ) {
        Ok(records) => {
            let records = runtime_gateway_admin_filter_ledger_records(records, shared, admin_auth);
            runtime_gateway_admin_csv_response(runtime_gateway_billing_summary_csv(
                &runtime_gateway_billing_summary_payload(shared, &records),
            ))
        }
        Err(_err) => build_runtime_proxy_json_error_response(
            500,
            "gateway_billing_summary_load_failed",
            "gateway billing summary could not be loaded",
        ),
    }
}

fn runtime_gateway_admin_filter_ledger_records(
    records: Vec<RuntimeGatewayBillingLedgerEntry>,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> Vec<RuntimeGatewayBillingLedgerEntry> {
    let entries = shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| entries.clone())
        .unwrap_or_default();
    records
        .into_iter()
        .filter(|record| runtime_gateway_admin_can_access_ledger_key(record, &entries, admin_auth))
        .collect()
}

fn runtime_gateway_admin_can_access_ledger_key(
    record: &RuntimeGatewayBillingLedgerEntry,
    entries: &[RuntimeGatewayVirtualKeyEntry],
    admin_auth: &RuntimeGatewayAdminAuth,
) -> bool {
    if !admin_auth.can_access_key(&record.key_name) {
        return false;
    }
    if runtime_gateway_admin_auth_is_unscoped(admin_auth) {
        return true;
    }
    if runtime_gateway_ledger_record_has_scope_snapshot(record) {
        return admin_auth.governance_scope().matches(
            record.tenant_id.as_deref(),
            record.team_id.as_deref(),
            record.project_id.as_deref(),
            record.user_id.as_deref(),
            record.budget_id.as_deref(),
        );
    }
    entries
        .iter()
        .find(|entry| entry.key.name.eq_ignore_ascii_case(&record.key_name))
        .map(|entry| {
            runtime_gateway_admin_auth_matches_entry(admin_auth, entry)
                && admin_auth.can_access_key(&entry.key.name)
        })
        .unwrap_or(false)
}

fn runtime_gateway_ledger_record_has_scope_snapshot(
    record: &RuntimeGatewayBillingLedgerEntry,
) -> bool {
    record.tenant_id.is_some()
        || record.team_id.is_some()
        || record.project_id.is_some()
        || record.user_id.is_some()
        || record.budget_id.is_some()
}

fn runtime_gateway_billing_summary_payload(
    shared: &RuntimeLocalRewriteProxyShared,
    records: &[RuntimeGatewayBillingLedgerEntry],
) -> serde_json::Value {
    let summary_records = records
        .iter()
        .map(runtime_gateway_billing_summary_record)
        .collect::<Vec<_>>();
    runtime_gateway_billing_summary_payload_from_records(
        shared.gateway_state_store.label(),
        shared
            .gateway_state_store
            .ledger_path()
            .display()
            .to_string(),
        &summary_records,
        &runtime_gateway_billing_summary_key_dimensions(shared),
    )
}

fn runtime_gateway_billing_summary_record(
    record: &RuntimeGatewayBillingLedgerEntry,
) -> RuntimeGatewayBillingSummaryRecord {
    RuntimeGatewayBillingSummaryRecord {
        phase: record.phase.clone(),
        key_name: record.key_name.clone(),
        tenant_id: record.tenant_id.clone(),
        team_id: record.team_id.clone(),
        project_id: record.project_id.clone(),
        user_id: record.user_id.clone(),
        budget_id: record.budget_id.clone(),
        model: record.model.clone(),
        input_tokens: record.input_tokens,
        estimated_cost_microusd: record.estimated_cost_microusd,
        created_at_epoch: record.created_at_epoch,
        response_status: record.response_status,
        response_bytes: record.response_bytes,
        output_tokens: record.output_tokens,
        final_cost_microusd: record.final_cost_microusd,
        reconciled_at_epoch: record.reconciled_at_epoch,
    }
}

fn runtime_gateway_billing_summary_key_dimensions(
    shared: &RuntimeLocalRewriteProxyShared,
) -> BTreeMap<String, RuntimeGatewayBillingSummaryKeyDimensions> {
    shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| {
            entries
                .iter()
                .map(|entry| {
                    (
                        entry.key.name.to_ascii_lowercase(),
                        RuntimeGatewayBillingSummaryKeyDimensions {
                            tenant_id: entry.key.tenant_id.clone(),
                            team_id: entry.key.team_id.clone(),
                            project_id: entry.key.project_id.clone(),
                            user_id: entry.key.user_id.clone(),
                            budget_id: entry.key.budget_id.clone(),
                        },
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scoped_admin(tenant_id: &str) -> RuntimeGatewayAdminAuth {
        RuntimeGatewayAdminAuth {
            name: "tenant-admin".to_string(),
            role: RuntimeGatewayAdminRole::Viewer,
            tenant_id: Some(tenant_id.to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        }
    }

    fn ledger_record(tenant_id: Option<&str>) -> RuntimeGatewayBillingLedgerEntry {
        RuntimeGatewayBillingLedgerEntry {
            object: "gateway.billing_ledger_entry".to_string(),
            phase: "request".to_string(),
            request_id: None,
            request: 1,
            call_id: "prodex-call".to_string(),
            key_name: "deleted-key".to_string(),
            tenant_id: tenant_id.map(str::to_string),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            model: "gpt-5".to_string(),
            minute_epoch: 10,
            input_tokens: 100,
            estimated_cost_microusd: None,
            estimated_cost_usd: None,
            created_at_epoch: 20,
            response_status: None,
            response_bytes: None,
            output_tokens: None,
            final_cost_microusd: None,
            final_cost_usd: None,
            reconciled_at_epoch: None,
        }
    }

    #[test]
    fn scoped_ledger_filter_uses_record_scope_snapshot_when_key_is_absent() {
        assert!(runtime_gateway_admin_can_access_ledger_key(
            &ledger_record(Some("tenant-a")),
            &[],
            &scoped_admin("tenant-a"),
        ));
        assert!(!runtime_gateway_admin_can_access_ledger_key(
            &ledger_record(Some("tenant-b")),
            &[],
            &scoped_admin("tenant-a"),
        ));
    }

    #[test]
    fn scoped_ledger_filter_keeps_legacy_records_without_snapshot_hidden_when_key_is_absent() {
        assert!(!runtime_gateway_admin_can_access_ledger_key(
            &ledger_record(None),
            &[],
            &scoped_admin("tenant-a"),
        ));
    }

    #[test]
    fn admin_ledger_exports_are_bounded() {
        assert!(RUNTIME_GATEWAY_ADMIN_LEDGER_EXPORT_LIMIT >= 1000);
        assert!(RUNTIME_GATEWAY_ADMIN_LEDGER_EXPORT_LIMIT < usize::MAX);
    }
}
