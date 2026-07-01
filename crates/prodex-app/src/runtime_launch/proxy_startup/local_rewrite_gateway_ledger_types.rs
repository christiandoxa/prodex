use prodex_provider_core::microusd_to_usd;
use serde::{Deserialize, Serialize};

use super::local_rewrite_gateway_usage_backend::RuntimeGatewayVirtualKeyUsageDelta;
use super::provider_bridge::RuntimeProviderGatewaySpendEvent;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RuntimeGatewayBillingLedgerEntry {
    pub(super) object: String,
    pub(super) phase: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) request_id: Option<String>,
    pub(super) request: u64,
    pub(super) call_id: String,
    pub(super) key_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) tenant_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) team_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) budget_id: Option<String>,
    pub(super) model: String,
    pub(super) minute_epoch: u64,
    pub(super) input_tokens: u64,
    pub(super) estimated_cost_microusd: Option<u64>,
    pub(super) estimated_cost_usd: Option<f64>,
    pub(super) created_at_epoch: u64,
    #[serde(default)]
    pub(super) response_status: Option<u16>,
    #[serde(default)]
    pub(super) response_bytes: Option<u64>,
    #[serde(default)]
    pub(super) output_tokens: Option<u64>,
    #[serde(default)]
    pub(super) final_cost_microusd: Option<u64>,
    #[serde(default)]
    pub(super) final_cost_usd: Option<f64>,
    #[serde(default)]
    pub(super) reconciled_at_epoch: Option<u64>,
}

pub(super) fn runtime_gateway_billing_ledger_entry_from_delta(
    delta: &RuntimeGatewayVirtualKeyUsageDelta,
) -> RuntimeGatewayBillingLedgerEntry {
    RuntimeGatewayBillingLedgerEntry {
        object: "gateway.billing_ledger_entry".to_string(),
        phase: "request".to_string(),
        request_id: Some(delta.typed_request_id.clone()),
        request: delta.request_id,
        call_id: delta.call_id.clone(),
        key_name: delta.key_name.clone(),
        tenant_id: delta.tenant_id.clone(),
        team_id: delta.team_id.clone(),
        project_id: delta.project_id.clone(),
        user_id: delta.user_id.clone(),
        budget_id: delta.budget_id.clone(),
        model: delta.model.clone(),
        minute_epoch: delta.minute_epoch,
        input_tokens: delta.input_tokens,
        estimated_cost_microusd: delta.estimated_cost_microusd,
        estimated_cost_usd: delta.estimated_cost_microusd.map(microusd_to_usd),
        created_at_epoch: delta.created_at_epoch,
        response_status: None,
        response_bytes: None,
        output_tokens: None,
        final_cost_microusd: None,
        final_cost_usd: None,
        reconciled_at_epoch: None,
    }
}

pub(super) fn runtime_gateway_apply_response_to_ledger_entry(
    entry: &mut RuntimeGatewayBillingLedgerEntry,
    event: &RuntimeProviderGatewaySpendEvent,
    reconciled_at_epoch: u64,
) {
    entry.response_status = Some(event.status);
    entry.response_bytes = event.response_bytes.map(|value| value as u64);
    entry.output_tokens = event.output_tokens;
    entry.final_cost_usd = event.cost_usd;
    entry.final_cost_microusd = runtime_gateway_usd_to_microusd(event.cost_usd);
    entry.reconciled_at_epoch = Some(reconciled_at_epoch);
}

pub(super) fn runtime_gateway_usd_to_microusd(value: Option<f64>) -> Option<u64> {
    let value = value?;
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    u64::try_from((value * 1_000_000.0).round() as i128).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_entry_from_delta_sets_request_phase_and_cost() {
        let call_id = format!("prodex-{}", prodex_domain::CallId::new());
        let typed_request_id = format!("prodex-{}", prodex_domain::RequestId::new());
        let entry =
            runtime_gateway_billing_ledger_entry_from_delta(&RuntimeGatewayVirtualKeyUsageDelta {
                request_id: 42,
                typed_request_id: typed_request_id.clone(),
                call_id: call_id.clone(),
                key_name: "alpha".to_string(),
                tenant_id: Some("tenant-a".to_string()),
                team_id: Some("platform".to_string()),
                project_id: None,
                user_id: None,
                budget_id: Some("budget-a".to_string()),
                model: "gpt-5".to_string(),
                minute_epoch: 10,
                input_tokens: 100,
                estimated_cost_microusd: Some(250_000),
                created_at_epoch: 20,
            });

        assert_eq!(entry.object, "gateway.billing_ledger_entry");
        assert_eq!(entry.phase, "request");
        assert_eq!(entry.request_id.as_deref(), Some(typed_request_id.as_str()));
        let request_id = entry
            .request_id
            .as_deref()
            .and_then(|id| id.strip_prefix("prodex-"))
            .expect("ledger request id should keep prodex prefix");
        assert_eq!(
            request_id
                .parse::<prodex_domain::RequestId>()
                .unwrap()
                .as_uuid()
                .get_version_num(),
            7
        );
        assert_eq!(entry.call_id, call_id);
        assert_eq!(entry.tenant_id.as_deref(), Some("tenant-a"));
        assert_eq!(entry.team_id.as_deref(), Some("platform"));
        assert_eq!(entry.budget_id.as_deref(), Some("budget-a"));
        assert_eq!(entry.estimated_cost_usd, Some(0.25));
    }

    #[test]
    fn usd_to_microusd_rejects_invalid_values() {
        assert_eq!(runtime_gateway_usd_to_microusd(Some(1.25)), Some(1_250_000));
        assert_eq!(runtime_gateway_usd_to_microusd(Some(-1.0)), None);
        assert_eq!(runtime_gateway_usd_to_microusd(Some(f64::NAN)), None);
        assert_eq!(runtime_gateway_usd_to_microusd(None), None);
    }
}
