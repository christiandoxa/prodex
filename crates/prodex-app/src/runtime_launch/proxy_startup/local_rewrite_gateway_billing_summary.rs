use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;

#[derive(Clone)]
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

impl fmt::Debug for RuntimeGatewayBillingSummaryRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayBillingSummaryRecord")
            .field("phase", &self.phase)
            .field("key_name", &"<redacted>")
            .field("tenant_id", &redacted_option(&self.tenant_id))
            .field("team_id", &redacted_option(&self.team_id))
            .field("project_id", &redacted_option(&self.project_id))
            .field("user_id", &redacted_option(&self.user_id))
            .field("budget_id", &redacted_option(&self.budget_id))
            .field("model", &"<redacted>")
            .field("input_tokens", &"<redacted>")
            .field(
                "estimated_cost_microusd",
                &redacted_option(&self.estimated_cost_microusd),
            )
            .field("created_at_epoch", &"<redacted>")
            .field("response_status", &self.response_status)
            .field("response_bytes", &redacted_option(&self.response_bytes))
            .field("output_tokens", &redacted_option(&self.output_tokens))
            .field(
                "final_cost_microusd",
                &redacted_option(&self.final_cost_microusd),
            )
            .field(
                "reconciled_at_epoch",
                &redacted_option(&self.reconciled_at_epoch),
            )
            .finish()
    }
}

#[derive(Clone, Default)]
pub(super) struct RuntimeGatewayBillingSummaryKeyDimensions {
    pub(super) tenant_id: Option<String>,
    pub(super) team_id: Option<String>,
    pub(super) project_id: Option<String>,
    pub(super) user_id: Option<String>,
    pub(super) budget_id: Option<String>,
}

impl fmt::Debug for RuntimeGatewayBillingSummaryKeyDimensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayBillingSummaryKeyDimensions")
            .field("tenant_id", &redacted_option(&self.tenant_id))
            .field("team_id", &redacted_option(&self.team_id))
            .field("project_id", &redacted_option(&self.project_id))
            .field("user_id", &redacted_option(&self.user_id))
            .field("budget_id", &redacted_option(&self.budget_id))
            .finish()
    }
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

#[derive(Clone, Default, Serialize)]
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

impl fmt::Debug for RuntimeGatewayBillingSummaryBucket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGatewayBillingSummaryBucket")
            .field("key_name", &redacted_option(&self.key_name))
            .field("model", &redacted_option(&self.model))
            .field("tenant_id", &redacted_option(&self.tenant_id))
            .field("team_id", &redacted_option(&self.team_id))
            .field("project_id", &redacted_option(&self.project_id))
            .field("user_id", &redacted_option(&self.user_id))
            .field("budget_id", &redacted_option(&self.budget_id))
            .field("requests", &"<redacted>")
            .field("successful_requests", &"<redacted>")
            .field("failed_requests", &"<redacted>")
            .field("unreconciled_requests", &"<redacted>")
            .field("input_tokens", &"<redacted>")
            .field("output_tokens", &"<redacted>")
            .field("response_bytes", &"<redacted>")
            .field("estimated_cost_microusd", &"<redacted>")
            .field("estimated_cost_usd", &"<redacted>")
            .field("final_cost_microusd", &"<redacted>")
            .field("final_cost_usd", &"<redacted>")
            .field(
                "first_created_at_epoch",
                &redacted_option(&self.first_created_at_epoch),
            )
            .field(
                "last_created_at_epoch",
                &redacted_option(&self.last_created_at_epoch),
            )
            .field(
                "last_reconciled_at_epoch",
                &redacted_option(&self.last_reconciled_at_epoch),
            )
            .finish()
    }
}

fn redacted_option<T>(value: &Option<T>) -> Option<&'static str> {
    value.as_ref().map(|_| "<redacted>")
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

    #[test]
    fn billing_summary_debug_output_redacts_sensitive_fields() {
        let record = RuntimeGatewayBillingSummaryRecord {
            phase: "request".to_string(),
            key_name: "sk-summary-secret".to_string(),
            tenant_id: Some("tenant-summary-secret".to_string()),
            team_id: Some("team-summary-secret".to_string()),
            project_id: Some("project-summary-secret".to_string()),
            user_id: Some("user-summary-secret".to_string()),
            budget_id: Some("budget-summary-secret".to_string()),
            model: "gpt-summary-secret".to_string(),
            input_tokens: 123,
            estimated_cost_microusd: Some(456),
            created_at_epoch: 1_700_000_000,
            response_status: Some(200),
            response_bytes: Some(789),
            output_tokens: Some(321),
            final_cost_microusd: Some(654),
            reconciled_at_epoch: Some(1_700_000_001),
        };
        let dimensions = RuntimeGatewayBillingSummaryKeyDimensions {
            tenant_id: Some("tenant-dimension-secret".to_string()),
            team_id: Some("team-dimension-secret".to_string()),
            project_id: Some("project-dimension-secret".to_string()),
            user_id: Some("user-dimension-secret".to_string()),
            budget_id: Some("budget-dimension-secret".to_string()),
        };
        let mut bucket = RuntimeGatewayBillingSummaryBucket::with_key_model(
            Some("sk-bucket-secret".to_string()),
            Some("gpt-bucket-secret".to_string()),
        );
        bucket.tenant_id = Some("tenant-bucket-secret".to_string());
        bucket.team_id = Some("team-bucket-secret".to_string());
        bucket.project_id = Some("project-bucket-secret".to_string());
        bucket.user_id = Some("user-bucket-secret".to_string());
        bucket.budget_id = Some("budget-bucket-secret".to_string());
        bucket.record(&record);

        let rendered = format!("{record:?}\n{dimensions:?}\n{bucket:?}");
        assert!(rendered.contains("RuntimeGatewayBillingSummaryRecord"));
        assert!(rendered.contains("RuntimeGatewayBillingSummaryKeyDimensions"));
        assert!(rendered.contains("RuntimeGatewayBillingSummaryBucket"));
        assert!(rendered.contains("response_status: Some(200)"));
        assert!(rendered.contains("<redacted>"));
        for raw in [
            "sk-summary-secret",
            "tenant-summary-secret",
            "team-summary-secret",
            "project-summary-secret",
            "user-summary-secret",
            "budget-summary-secret",
            "gpt-summary-secret",
            "tenant-dimension-secret",
            "team-dimension-secret",
            "project-dimension-secret",
            "user-dimension-secret",
            "budget-dimension-secret",
            "sk-bucket-secret",
            "gpt-bucket-secret",
            "tenant-bucket-secret",
            "team-bucket-secret",
            "project-bucket-secret",
            "user-bucket-secret",
            "budget-bucket-secret",
        ] {
            assert!(!rendered.contains(raw), "{rendered}");
        }
    }
}
