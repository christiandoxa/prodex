use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Clone, Debug)]
pub(super) struct RuntimeGatewayBillingSummaryRecord {
    pub(super) phase: String,
    pub(super) key_name: String,
    pub(super) tenant_id: Option<String>,
    pub(super) team_id: Option<String>,
    pub(super) project_id: Option<String>,
    pub(super) user_id: Option<String>,
    pub(super) budget_id: Option<String>,
    pub(super) model: String,
    pub(super) input_tokens: u64,
    pub(super) estimated_cost_microusd: Option<u64>,
    pub(super) created_at_epoch: u64,
    pub(super) response_status: Option<u16>,
    pub(super) response_bytes: Option<u64>,
    pub(super) output_tokens: Option<u64>,
    pub(super) final_cost_microusd: Option<u64>,
    pub(super) reconciled_at_epoch: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct RuntimeGatewayBillingSummaryKeyDimensions {
    pub(super) tenant_id: Option<String>,
    pub(super) team_id: Option<String>,
    pub(super) project_id: Option<String>,
    pub(super) user_id: Option<String>,
    pub(super) budget_id: Option<String>,
}

impl RuntimeGatewayBillingSummaryKeyDimensions {
    fn has_any(&self) -> bool {
        self.tenant_id.is_some()
            || self.team_id.is_some()
            || self.project_id.is_some()
            || self.user_id.is_some()
            || self.budget_id.is_some()
    }
}

#[derive(Clone, Debug, Default, Serialize)]
struct RuntimeGatewayBillingSummaryBucket {
    key_name: Option<String>,
    model: Option<String>,
    tenant_id: Option<String>,
    team_id: Option<String>,
    project_id: Option<String>,
    user_id: Option<String>,
    budget_id: Option<String>,
    requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    unreconciled_requests: u64,
    input_tokens: u64,
    output_tokens: u64,
    response_bytes: u64,
    estimated_cost_microusd: u64,
    estimated_cost_usd: f64,
    final_cost_microusd: u64,
    final_cost_usd: f64,
    first_created_at_epoch: Option<u64>,
    last_created_at_epoch: Option<u64>,
    last_reconciled_at_epoch: Option<u64>,
}

impl RuntimeGatewayBillingSummaryBucket {
    fn with_key_model(key_name: Option<String>, model: Option<String>) -> Self {
        Self {
            key_name,
            model,
            ..Self::default()
        }
    }

    fn with_dimension(field: &str, value: String) -> Self {
        let mut bucket = Self::default();
        match field {
            "tenant_id" => bucket.tenant_id = Some(value),
            "team_id" => bucket.team_id = Some(value),
            "project_id" => bucket.project_id = Some(value),
            "user_id" => bucket.user_id = Some(value),
            "budget_id" => bucket.budget_id = Some(value),
            _ => {}
        }
        bucket
    }

    fn record(&mut self, entry: &RuntimeGatewayBillingSummaryRecord) {
        self.requests = self.requests.saturating_add(1);
        match entry.response_status {
            Some(status) if (200..300).contains(&status) => {
                self.successful_requests = self.successful_requests.saturating_add(1);
            }
            Some(_) => {
                self.failed_requests = self.failed_requests.saturating_add(1);
            }
            None => {
                self.unreconciled_requests = self.unreconciled_requests.saturating_add(1);
            }
        }
        self.input_tokens = self.input_tokens.saturating_add(entry.input_tokens);
        self.output_tokens = self
            .output_tokens
            .saturating_add(entry.output_tokens.unwrap_or_default());
        self.response_bytes = self
            .response_bytes
            .saturating_add(entry.response_bytes.unwrap_or_default());
        self.estimated_cost_microusd = self
            .estimated_cost_microusd
            .saturating_add(entry.estimated_cost_microusd.unwrap_or_default());
        self.final_cost_microusd = self
            .final_cost_microusd
            .saturating_add(entry.final_cost_microusd.unwrap_or_default());
        self.estimated_cost_usd = microusd_to_usd(self.estimated_cost_microusd);
        self.final_cost_usd = microusd_to_usd(self.final_cost_microusd);
        self.first_created_at_epoch = Some(
            self.first_created_at_epoch
                .map(|current| current.min(entry.created_at_epoch))
                .unwrap_or(entry.created_at_epoch),
        );
        self.last_created_at_epoch = Some(
            self.last_created_at_epoch
                .map(|current| current.max(entry.created_at_epoch))
                .unwrap_or(entry.created_at_epoch),
        );
        if let Some(reconciled_at_epoch) = entry.reconciled_at_epoch {
            self.last_reconciled_at_epoch = Some(
                self.last_reconciled_at_epoch
                    .map(|current| current.max(reconciled_at_epoch))
                    .unwrap_or(reconciled_at_epoch),
            );
        }
    }
}

pub(super) fn runtime_gateway_billing_summary_payload(
    state_backend: &str,
    ledger_path: String,
    records: &[RuntimeGatewayBillingSummaryRecord],
    key_dimensions: &BTreeMap<String, RuntimeGatewayBillingSummaryKeyDimensions>,
) -> serde_json::Value {
    let mut totals = RuntimeGatewayBillingSummaryBucket::default();
    let mut by_key: BTreeMap<String, RuntimeGatewayBillingSummaryBucket> = BTreeMap::new();
    let mut by_model: BTreeMap<String, RuntimeGatewayBillingSummaryBucket> = BTreeMap::new();
    let mut by_key_model: BTreeMap<(String, String), RuntimeGatewayBillingSummaryBucket> =
        BTreeMap::new();
    let mut by_tenant: BTreeMap<String, RuntimeGatewayBillingSummaryBucket> = BTreeMap::new();
    let mut by_team: BTreeMap<String, RuntimeGatewayBillingSummaryBucket> = BTreeMap::new();
    let mut by_project: BTreeMap<String, RuntimeGatewayBillingSummaryBucket> = BTreeMap::new();
    let mut by_user: BTreeMap<String, RuntimeGatewayBillingSummaryBucket> = BTreeMap::new();
    let mut by_budget: BTreeMap<String, RuntimeGatewayBillingSummaryBucket> = BTreeMap::new();
    for record in records.iter().filter(|record| record.phase == "request") {
        totals.record(record);
        by_key
            .entry(record.key_name.clone())
            .or_insert_with(|| {
                RuntimeGatewayBillingSummaryBucket::with_key_model(
                    Some(record.key_name.clone()),
                    None,
                )
            })
            .record(record);
        by_model
            .entry(record.model.clone())
            .or_insert_with(|| {
                RuntimeGatewayBillingSummaryBucket::with_key_model(None, Some(record.model.clone()))
            })
            .record(record);
        by_key_model
            .entry((record.key_name.clone(), record.model.clone()))
            .or_insert_with(|| {
                RuntimeGatewayBillingSummaryBucket::with_key_model(
                    Some(record.key_name.clone()),
                    Some(record.model.clone()),
                )
            })
            .record(record);
        let ledger_dimensions = RuntimeGatewayBillingSummaryKeyDimensions {
            tenant_id: record.tenant_id.clone(),
            team_id: record.team_id.clone(),
            project_id: record.project_id.clone(),
            user_id: record.user_id.clone(),
            budget_id: record.budget_id.clone(),
        };
        let dimensions = if ledger_dimensions.has_any() {
            Some(&ledger_dimensions)
        } else {
            key_dimensions.get(&record.key_name.to_ascii_lowercase())
        };
        let Some(dimensions) = dimensions else {
            continue;
        };
        for (value, field, buckets) in [
            (dimensions.tenant_id.as_deref(), "tenant_id", &mut by_tenant),
            (dimensions.team_id.as_deref(), "team_id", &mut by_team),
            (
                dimensions.project_id.as_deref(),
                "project_id",
                &mut by_project,
            ),
            (dimensions.user_id.as_deref(), "user_id", &mut by_user),
            (dimensions.budget_id.as_deref(), "budget_id", &mut by_budget),
        ] {
            if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
                buckets
                    .entry(value.to_string())
                    .or_insert_with(|| {
                        RuntimeGatewayBillingSummaryBucket::with_dimension(field, value.to_string())
                    })
                    .record(record);
            }
        }
    }
    serde_json::json!({
        "object": "gateway.billing_summary",
        "state_backend": state_backend,
        "ledger_path": ledger_path,
        "record_count": records.len(),
        "totals": totals,
        "by_key": by_key.into_values().collect::<Vec<_>>(),
        "by_model": by_model.into_values().collect::<Vec<_>>(),
        "by_key_model": by_key_model.into_values().collect::<Vec<_>>(),
        "by_tenant": by_tenant.into_values().collect::<Vec<_>>(),
        "by_team": by_team.into_values().collect::<Vec<_>>(),
        "by_project": by_project.into_values().collect::<Vec<_>>(),
        "by_user": by_user.into_values().collect::<Vec<_>>(),
        "by_budget": by_budget.into_values().collect::<Vec<_>>(),
    })
}

fn microusd_to_usd(value: u64) -> f64 {
    value as f64 / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_groups_records_by_key_model_and_governance_dimensions() {
        let records = vec![RuntimeGatewayBillingSummaryRecord {
            phase: "request".to_string(),
            key_name: "alpha".to_string(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            model: "gpt-5.4".to_string(),
            input_tokens: 100,
            estimated_cost_microusd: Some(250_000),
            created_at_epoch: 10,
            response_status: Some(200),
            response_bytes: Some(1234),
            output_tokens: Some(50),
            final_cost_microusd: Some(500_000),
            reconciled_at_epoch: Some(20),
        }];
        let mut dimensions = BTreeMap::new();
        dimensions.insert(
            "alpha".to_ascii_lowercase(),
            RuntimeGatewayBillingSummaryKeyDimensions {
                team_id: Some("platform".to_string()),
                budget_id: Some("budget-a".to_string()),
                ..Default::default()
            },
        );
        let summary = runtime_gateway_billing_summary_payload(
            "file",
            "/tmp/ledger.jsonl".to_string(),
            &records,
            &dimensions,
        );
        assert_eq!(summary["totals"]["requests"], 1);
        assert_eq!(summary["totals"]["successful_requests"], 1);
        assert_eq!(summary["totals"]["final_cost_usd"], 0.5);
        assert_eq!(summary["by_key"][0]["key_name"], "alpha");
        assert_eq!(summary["by_model"][0]["model"], "gpt-5.4");
        assert_eq!(summary["by_team"][0]["team_id"], "platform");
        assert_eq!(summary["by_budget"][0]["budget_id"], "budget-a");
    }

    #[test]
    fn summary_prefers_ledger_dimension_snapshot_over_current_key_store() {
        let records = vec![RuntimeGatewayBillingSummaryRecord {
            phase: "request".to_string(),
            key_name: "alpha".to_string(),
            tenant_id: Some("tenant-ledger".to_string()),
            team_id: Some("historical".to_string()),
            project_id: None,
            user_id: None,
            budget_id: Some("budget-ledger".to_string()),
            model: "gpt-5.4".to_string(),
            input_tokens: 100,
            estimated_cost_microusd: Some(250_000),
            created_at_epoch: 10,
            response_status: Some(200),
            response_bytes: None,
            output_tokens: None,
            final_cost_microusd: None,
            reconciled_at_epoch: None,
        }];
        let mut dimensions = BTreeMap::new();
        dimensions.insert(
            "alpha".to_ascii_lowercase(),
            RuntimeGatewayBillingSummaryKeyDimensions {
                tenant_id: Some("tenant-current".to_string()),
                team_id: Some("current".to_string()),
                budget_id: Some("budget-current".to_string()),
                ..Default::default()
            },
        );
        let summary = runtime_gateway_billing_summary_payload(
            "file",
            "/tmp/ledger.jsonl".to_string(),
            &records,
            &dimensions,
        );

        assert_eq!(summary["by_tenant"][0]["tenant_id"], "tenant-ledger");
        assert_eq!(summary["by_team"][0]["team_id"], "historical");
        assert_eq!(summary["by_budget"][0]["budget_id"], "budget-ledger");
    }
}
