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
        Err(err) => build_runtime_proxy_json_error_response(
            500,
            "gateway_billing_ledger_load_failed",
            &err.to_string(),
        ),
    }
}

pub(super) fn runtime_gateway_admin_ledger_csv_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    match runtime_gateway_billing_ledger_load(&shared.gateway_state_store, usize::MAX) {
        Ok(records) => {
            let records = runtime_gateway_admin_filter_ledger_records(records, shared, admin_auth);
            runtime_gateway_admin_csv_response(runtime_gateway_billing_ledger_csv(&records))
        }
        Err(err) => build_runtime_proxy_json_error_response(
            500,
            "gateway_billing_ledger_load_failed",
            &err.to_string(),
        ),
    }
}

pub(super) fn runtime_gateway_admin_ledger_summary_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    match runtime_gateway_billing_ledger_load(&shared.gateway_state_store, usize::MAX) {
        Ok(records) => {
            let records = runtime_gateway_admin_filter_ledger_records(records, shared, admin_auth);
            runtime_gateway_admin_json_response(
                200,
                runtime_gateway_billing_summary_payload(shared, &records),
            )
        }
        Err(err) => build_runtime_proxy_json_error_response(
            500,
            "gateway_billing_summary_load_failed",
            &err.to_string(),
        ),
    }
}

pub(super) fn runtime_gateway_admin_ledger_summary_csv_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    match runtime_gateway_billing_ledger_load(&shared.gateway_state_store, usize::MAX) {
        Ok(records) => {
            let records = runtime_gateway_admin_filter_ledger_records(records, shared, admin_auth);
            runtime_gateway_admin_csv_response(runtime_gateway_billing_summary_csv(
                &runtime_gateway_billing_summary_payload(shared, &records),
            ))
        }
        Err(err) => build_runtime_proxy_json_error_response(
            500,
            "gateway_billing_summary_load_failed",
            &err.to_string(),
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
    entries
        .iter()
        .find(|entry| entry.key.name.eq_ignore_ascii_case(&record.key_name))
        .map(|entry| {
            runtime_gateway_admin_auth_matches_entry(admin_auth, entry)
                && admin_auth.can_access_key(&entry.key.name)
        })
        .unwrap_or(false)
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
