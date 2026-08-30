use std::collections::{BTreeMap, btree_map::Entry};
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

    fn with_numeric(mut self, numeric: RuntimeGatewayBillingSummaryNumericBucket) -> Self {
        self.requests = numeric.requests;
        self.successful_requests = numeric.successful_requests;
        self.failed_requests = numeric.failed_requests;
        self.unreconciled_requests = numeric.unreconciled_requests;
        self.input_tokens = numeric.input_tokens;
        self.output_tokens = numeric.output_tokens;
        self.response_bytes = numeric.response_bytes;
        self.estimated_cost_microusd = numeric.estimated_cost_microusd;
        self.estimated_cost_usd = microusd_to_usd(numeric.estimated_cost_microusd);
        self.final_cost_microusd = numeric.final_cost_microusd;
        self.final_cost_usd = microusd_to_usd(numeric.final_cost_microusd);
        self.first_created_at_epoch =
            (numeric.first_created_at_present == 1).then_some(numeric.first_created_at_epoch);
        self.last_created_at_epoch =
            (numeric.first_created_at_present == 1).then_some(numeric.last_created_at_epoch);
        self.last_reconciled_at_epoch =
            (numeric.last_reconciled_at_present == 1).then_some(numeric.last_reconciled_at_epoch);
        self
    }
}

const RUNTIME_GATEWAY_BILLING_SUMMARY_CATEGORY_COUNT: usize = 9;
const RUNTIME_GATEWAY_BILLING_SUMMARY_TOTAL: usize = 0;
const RUNTIME_GATEWAY_BILLING_SUMMARY_BY_KEY: usize = 1;
const RUNTIME_GATEWAY_BILLING_SUMMARY_BY_MODEL: usize = 2;
const RUNTIME_GATEWAY_BILLING_SUMMARY_BY_KEY_MODEL: usize = 3;
const RUNTIME_GATEWAY_BILLING_SUMMARY_BY_TENANT: usize = 4;
const RUNTIME_GATEWAY_BILLING_SUMMARY_BY_TEAM: usize = 5;
const RUNTIME_GATEWAY_BILLING_SUMMARY_BY_PROJECT: usize = 6;
const RUNTIME_GATEWAY_BILLING_SUMMARY_BY_USER: usize = 7;
const RUNTIME_GATEWAY_BILLING_SUMMARY_BY_BUDGET: usize = 8;

#[derive(Clone, Copy, Debug, Default)]
struct RuntimeGatewayBillingSummaryNumericInput {
    bucket_ids: [i64; RUNTIME_GATEWAY_BILLING_SUMMARY_CATEGORY_COUNT],
    response_status: i64,
    response_status_present: i64,
    input_tokens: u64,
    output_tokens: u64,
    response_bytes: u64,
    estimated_cost_microusd: u64,
    final_cost_microusd: u64,
    created_at_epoch: u64,
    reconciled_at_epoch: u64,
    reconciled_at_present: i64,
}

#[derive(Clone, Copy, Debug, Default)]
struct RuntimeGatewayBillingSummaryNumericBucket {
    requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    unreconciled_requests: u64,
    input_tokens: u64,
    output_tokens: u64,
    response_bytes: u64,
    estimated_cost_microusd: u64,
    final_cost_microusd: u64,
    first_created_at_epoch: u64,
    first_created_at_present: i64,
    last_created_at_epoch: u64,
    last_reconciled_at_epoch: u64,
    last_reconciled_at_present: i64,
}

fn runtime_gateway_billing_summary_group_index<K: Ord>(
    groups: &mut BTreeMap<K, usize>,
    key: K,
    next_bucket: &mut usize,
) -> usize {
    match groups.entry(key) {
        Entry::Occupied(entry) => *entry.get(),
        Entry::Vacant(entry) => {
            let index = *next_bucket;
            *next_bucket += 1;
            entry.insert(index);
            index
        }
    }
}

pub(super) fn runtime_gateway_billing_summary_payload(
    state_backend: &str,
    ledger_path: String,
    records: &[RuntimeGatewayBillingSummaryRecord],
    key_dimensions: &BTreeMap<String, RuntimeGatewayBillingSummaryKeyDimensions>,
) -> serde_json::Value {
    let mut by_key = BTreeMap::new();
    let mut by_model = BTreeMap::new();
    let mut by_key_model = BTreeMap::new();
    let mut by_tenant = BTreeMap::new();
    let mut by_team = BTreeMap::new();
    let mut by_project = BTreeMap::new();
    let mut by_user = BTreeMap::new();
    let mut by_budget = BTreeMap::new();
    let mut next_bucket = 1;
    let mut numeric_inputs = Vec::with_capacity(records.len());
    for record in records.iter().filter(|record| record.phase == "request") {
        let mut bucket_ids = [-1_i64; RUNTIME_GATEWAY_BILLING_SUMMARY_CATEGORY_COUNT];
        bucket_ids[RUNTIME_GATEWAY_BILLING_SUMMARY_TOTAL] = 0;
        bucket_ids[RUNTIME_GATEWAY_BILLING_SUMMARY_BY_KEY] =
            runtime_gateway_billing_summary_group_index(
                &mut by_key,
                record.key_name.clone(),
                &mut next_bucket,
            ) as i64;
        bucket_ids[RUNTIME_GATEWAY_BILLING_SUMMARY_BY_MODEL] =
            runtime_gateway_billing_summary_group_index(
                &mut by_model,
                record.model.clone(),
                &mut next_bucket,
            ) as i64;
        bucket_ids[RUNTIME_GATEWAY_BILLING_SUMMARY_BY_KEY_MODEL] =
            runtime_gateway_billing_summary_group_index(
                &mut by_key_model,
                (record.key_name.clone(), record.model.clone()),
                &mut next_bucket,
            ) as i64;
        let ledger_dimensions = RuntimeGatewayBillingSummaryKeyDimensions {
            tenant_id: record.tenant_id.clone(),
            team_id: record.team_id.clone(),
            project_id: record.project_id.clone(),
            user_id: record.user_id.clone(),
            budget_id: record.budget_id.clone(),
        };
        let dimensions = if ledger_dimensions.has_any() {
            ledger_dimensions
        } else {
            key_dimensions
                .get(&record.key_name.to_ascii_lowercase())
                .cloned()
                .unwrap_or_default()
        };
        for (slot, value, groups) in [
            (
                RUNTIME_GATEWAY_BILLING_SUMMARY_BY_TENANT,
                dimensions.tenant_id.as_deref(),
                &mut by_tenant,
            ),
            (
                RUNTIME_GATEWAY_BILLING_SUMMARY_BY_TEAM,
                dimensions.team_id.as_deref(),
                &mut by_team,
            ),
            (
                RUNTIME_GATEWAY_BILLING_SUMMARY_BY_PROJECT,
                dimensions.project_id.as_deref(),
                &mut by_project,
            ),
            (
                RUNTIME_GATEWAY_BILLING_SUMMARY_BY_USER,
                dimensions.user_id.as_deref(),
                &mut by_user,
            ),
            (
                RUNTIME_GATEWAY_BILLING_SUMMARY_BY_BUDGET,
                dimensions.budget_id.as_deref(),
                &mut by_budget,
            ),
        ] {
            if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
                bucket_ids[slot] = runtime_gateway_billing_summary_group_index(
                    groups,
                    value.to_string(),
                    &mut next_bucket,
                ) as i64;
            }
        }
        numeric_inputs.push(RuntimeGatewayBillingSummaryNumericInput {
            bucket_ids,
            response_status: i64::from(record.response_status.unwrap_or_default()),
            response_status_present: i64::from(record.response_status.is_some()),
            input_tokens: record.input_tokens,
            output_tokens: record.output_tokens.unwrap_or_default(),
            response_bytes: record.response_bytes.unwrap_or_default(),
            estimated_cost_microusd: record.estimated_cost_microusd.unwrap_or_default(),
            final_cost_microusd: record.final_cost_microusd.unwrap_or_default(),
            created_at_epoch: record.created_at_epoch,
            reconciled_at_epoch: record.reconciled_at_epoch.unwrap_or_default(),
            reconciled_at_present: i64::from(record.reconciled_at_epoch.is_some()),
        });
    }
    let numeric = runtime_gateway_billing_summary_numeric_batch(&numeric_inputs, next_bucket);
    let totals = RuntimeGatewayBillingSummaryBucket::default().with_numeric(numeric[0]);
    let by_key = by_key
        .into_iter()
        .map(|(key, index)| {
            RuntimeGatewayBillingSummaryBucket::with_key_model(Some(key), None)
                .with_numeric(numeric[index])
        })
        .collect::<Vec<_>>();
    let by_model = by_model
        .into_iter()
        .map(|(model, index)| {
            RuntimeGatewayBillingSummaryBucket::with_key_model(None, Some(model))
                .with_numeric(numeric[index])
        })
        .collect::<Vec<_>>();
    let by_key_model = by_key_model
        .into_iter()
        .map(|((key, model), index)| {
            RuntimeGatewayBillingSummaryBucket::with_key_model(Some(key), Some(model))
                .with_numeric(numeric[index])
        })
        .collect::<Vec<_>>();
    let by_tenant =
        runtime_gateway_billing_summary_dimension_buckets(by_tenant, "tenant_id", &numeric);
    let by_team = runtime_gateway_billing_summary_dimension_buckets(by_team, "team_id", &numeric);
    let by_project =
        runtime_gateway_billing_summary_dimension_buckets(by_project, "project_id", &numeric);
    let by_user = runtime_gateway_billing_summary_dimension_buckets(by_user, "user_id", &numeric);
    let by_budget =
        runtime_gateway_billing_summary_dimension_buckets(by_budget, "budget_id", &numeric);
    serde_json::json!({
        "object": "gateway.billing_summary",
        "state_backend": state_backend,
        "ledger_path": ledger_path,
        "record_count": records.len(),
        "totals": totals,
        "by_key": by_key,
        "by_model": by_model,
        "by_key_model": by_key_model,
        "by_tenant": by_tenant,
        "by_team": by_team,
        "by_project": by_project,
        "by_user": by_user,
        "by_budget": by_budget,
    })
}

fn runtime_gateway_billing_summary_dimension_buckets(
    groups: BTreeMap<String, usize>,
    field: &str,
    numeric: &[RuntimeGatewayBillingSummaryNumericBucket],
) -> Vec<RuntimeGatewayBillingSummaryBucket> {
    groups
        .into_iter()
        .map(|(value, index)| {
            RuntimeGatewayBillingSummaryBucket::with_dimension(field, value)
                .with_numeric(numeric[index])
        })
        .collect()
}

fn runtime_gateway_billing_summary_numeric_batch(
    inputs: &[RuntimeGatewayBillingSummaryNumericInput],
    bucket_count: usize,
) -> Vec<RuntimeGatewayBillingSummaryNumericBucket> {
    #[cfg(feature = "mojo-core")]
    {
        let inputs = inputs
            .iter()
            .map(|input| prodex_mojo_core::rich::GatewayBillingSummaryInput {
                bucket_ids: input.bucket_ids,
                response_status: input.response_status,
                response_status_present: input.response_status_present,
                input_tokens: input.input_tokens,
                output_tokens: input.output_tokens,
                response_bytes: input.response_bytes,
                estimated_cost_microusd: input.estimated_cost_microusd,
                final_cost_microusd: input.final_cost_microusd,
                created_at_epoch: input.created_at_epoch,
                reconciled_at_epoch: input.reconciled_at_epoch,
                reconciled_at_present: input.reconciled_at_present,
            })
            .collect::<Vec<_>>();
        prodex_mojo_core::rich::gateway_billing_summary_batch(&inputs, bucket_count)
            .expect("Mojo gateway billing summary batch returned invalid structured result")
            .into_iter()
            .map(|bucket| RuntimeGatewayBillingSummaryNumericBucket {
                requests: bucket.requests,
                successful_requests: bucket.successful_requests,
                failed_requests: bucket.failed_requests,
                unreconciled_requests: bucket.unreconciled_requests,
                input_tokens: bucket.input_tokens,
                output_tokens: bucket.output_tokens,
                response_bytes: bucket.response_bytes,
                estimated_cost_microusd: bucket.estimated_cost_microusd,
                final_cost_microusd: bucket.final_cost_microusd,
                first_created_at_epoch: bucket.first_created_at_epoch,
                first_created_at_present: bucket.first_created_at_present,
                last_created_at_epoch: bucket.last_created_at_epoch,
                last_reconciled_at_epoch: bucket.last_reconciled_at_epoch,
                last_reconciled_at_present: bucket.last_reconciled_at_present,
            })
            .collect()
    }

    #[cfg(not(feature = "mojo-core"))]
    runtime_gateway_billing_summary_numeric_batch_rust(inputs, bucket_count)
}

#[cfg(not(feature = "mojo-core"))]
fn runtime_gateway_billing_summary_numeric_batch_rust(
    inputs: &[RuntimeGatewayBillingSummaryNumericInput],
    bucket_count: usize,
) -> Vec<RuntimeGatewayBillingSummaryNumericBucket> {
    let mut buckets = vec![RuntimeGatewayBillingSummaryNumericBucket::default(); bucket_count];
    for input in inputs {
        for bucket_id in input.bucket_ids {
            let Ok(bucket_id) = usize::try_from(bucket_id) else {
                continue;
            };
            let bucket = &mut buckets[bucket_id];
            bucket.requests = bucket.requests.saturating_add(1);
            match input.response_status_present {
                0 => bucket.unreconciled_requests = bucket.unreconciled_requests.saturating_add(1),
                _ if (200..300).contains(&input.response_status) => {
                    bucket.successful_requests = bucket.successful_requests.saturating_add(1)
                }
                _ => bucket.failed_requests = bucket.failed_requests.saturating_add(1),
            }
            bucket.input_tokens = bucket.input_tokens.saturating_add(input.input_tokens);
            bucket.output_tokens = bucket.output_tokens.saturating_add(input.output_tokens);
            bucket.response_bytes = bucket.response_bytes.saturating_add(input.response_bytes);
            bucket.estimated_cost_microusd = bucket
                .estimated_cost_microusd
                .saturating_add(input.estimated_cost_microusd);
            bucket.final_cost_microusd = bucket
                .final_cost_microusd
                .saturating_add(input.final_cost_microusd);
            if bucket.first_created_at_present == 0 {
                bucket.first_created_at_epoch = input.created_at_epoch;
                bucket.last_created_at_epoch = input.created_at_epoch;
                bucket.first_created_at_present = 1;
            } else {
                bucket.first_created_at_epoch =
                    bucket.first_created_at_epoch.min(input.created_at_epoch);
                bucket.last_created_at_epoch =
                    bucket.last_created_at_epoch.max(input.created_at_epoch);
            }
            if input.reconciled_at_present == 1 {
                if bucket.last_reconciled_at_present == 0 {
                    bucket.last_reconciled_at_epoch = input.reconciled_at_epoch;
                    bucket.last_reconciled_at_present = 1;
                } else {
                    bucket.last_reconciled_at_epoch = bucket
                        .last_reconciled_at_epoch
                        .max(input.reconciled_at_epoch);
                }
            }
        }
    }
    buckets
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
        bucket = bucket.with_numeric(RuntimeGatewayBillingSummaryNumericBucket {
            requests: 1,
            successful_requests: 1,
            input_tokens: record.input_tokens,
            output_tokens: record.output_tokens.unwrap_or_default(),
            response_bytes: record.response_bytes.unwrap_or_default(),
            estimated_cost_microusd: record.estimated_cost_microusd.unwrap_or_default(),
            final_cost_microusd: record.final_cost_microusd.unwrap_or_default(),
            first_created_at_epoch: record.created_at_epoch,
            first_created_at_present: 1,
            last_created_at_epoch: record.created_at_epoch,
            last_reconciled_at_epoch: record.reconciled_at_epoch.unwrap_or_default(),
            last_reconciled_at_present: i64::from(record.reconciled_at_epoch.is_some()),
            ..Default::default()
        });

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
