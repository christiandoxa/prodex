use serde::Serialize;

pub(super) fn runtime_gateway_billing_ledger_csv<T: Serialize>(records: &[T]) -> String {
    let mut csv = String::new();
    runtime_gateway_csv_row(
        &mut csv,
        [
            "call_id",
            "key_name",
            "model",
            "phase",
            "request",
            "created_at_epoch",
            "minute_epoch",
            "input_tokens",
            "output_tokens",
            "response_status",
            "response_bytes",
            "estimated_cost_microusd",
            "estimated_cost_usd",
            "final_cost_microusd",
            "final_cost_usd",
            "reconciled_at_epoch",
        ],
    );
    for record in records {
        let record = serde_json::to_value(record).unwrap_or_default();
        runtime_gateway_csv_row(
            &mut csv,
            &[
                runtime_gateway_json_csv_string(&record, "call_id"),
                runtime_gateway_json_csv_string(&record, "key_name"),
                runtime_gateway_json_csv_string(&record, "model"),
                runtime_gateway_json_csv_string(&record, "phase"),
                runtime_gateway_json_csv_u64(&record, "request"),
                runtime_gateway_json_csv_u64(&record, "created_at_epoch"),
                runtime_gateway_json_csv_u64(&record, "minute_epoch"),
                runtime_gateway_json_csv_u64(&record, "input_tokens"),
                runtime_gateway_json_csv_u64(&record, "output_tokens"),
                runtime_gateway_json_csv_u64(&record, "response_status"),
                runtime_gateway_json_csv_u64(&record, "response_bytes"),
                runtime_gateway_json_csv_u64(&record, "estimated_cost_microusd"),
                runtime_gateway_json_csv_f64(&record, "estimated_cost_usd"),
                runtime_gateway_json_csv_u64(&record, "final_cost_microusd"),
                runtime_gateway_json_csv_f64(&record, "final_cost_usd"),
                runtime_gateway_json_csv_u64(&record, "reconciled_at_epoch"),
            ],
        );
    }
    csv
}

pub(super) fn runtime_gateway_billing_summary_csv(summary: &serde_json::Value) -> String {
    let mut csv = String::new();
    runtime_gateway_csv_row(
        &mut csv,
        [
            "group",
            "key_name",
            "model",
            "team_id",
            "project_id",
            "user_id",
            "budget_id",
            "requests",
            "successful_requests",
            "failed_requests",
            "unreconciled_requests",
            "input_tokens",
            "output_tokens",
            "response_bytes",
            "estimated_cost_microusd",
            "estimated_cost_usd",
            "final_cost_microusd",
            "final_cost_usd",
            "first_created_at_epoch",
            "last_created_at_epoch",
            "last_reconciled_at_epoch",
        ],
    );
    if let Some(totals) = summary.get("totals") {
        runtime_gateway_billing_summary_csv_row(&mut csv, "totals", totals);
    }
    for group in [
        "by_key",
        "by_model",
        "by_key_model",
        "by_team",
        "by_project",
        "by_user",
        "by_budget",
    ] {
        if let Some(rows) = summary.get(group).and_then(serde_json::Value::as_array) {
            for row in rows {
                runtime_gateway_billing_summary_csv_row(&mut csv, group, row);
            }
        }
    }
    csv
}

fn runtime_gateway_billing_summary_csv_row(csv: &mut String, group: &str, row: &serde_json::Value) {
    runtime_gateway_csv_row(
        csv,
        &[
            group.to_string(),
            runtime_gateway_json_csv_string(row, "key_name"),
            runtime_gateway_json_csv_string(row, "model"),
            runtime_gateway_json_csv_string(row, "team_id"),
            runtime_gateway_json_csv_string(row, "project_id"),
            runtime_gateway_json_csv_string(row, "user_id"),
            runtime_gateway_json_csv_string(row, "budget_id"),
            runtime_gateway_json_csv_u64(row, "requests"),
            runtime_gateway_json_csv_u64(row, "successful_requests"),
            runtime_gateway_json_csv_u64(row, "failed_requests"),
            runtime_gateway_json_csv_u64(row, "unreconciled_requests"),
            runtime_gateway_json_csv_u64(row, "input_tokens"),
            runtime_gateway_json_csv_u64(row, "output_tokens"),
            runtime_gateway_json_csv_u64(row, "response_bytes"),
            runtime_gateway_json_csv_u64(row, "estimated_cost_microusd"),
            runtime_gateway_json_csv_f64(row, "estimated_cost_usd"),
            runtime_gateway_json_csv_u64(row, "final_cost_microusd"),
            runtime_gateway_json_csv_f64(row, "final_cost_usd"),
            runtime_gateway_json_csv_u64(row, "first_created_at_epoch"),
            runtime_gateway_json_csv_u64(row, "last_created_at_epoch"),
            runtime_gateway_json_csv_u64(row, "last_reconciled_at_epoch"),
        ],
    );
}

fn runtime_gateway_json_csv_string(row: &serde_json::Value, field: &str) -> String {
    row.get(field)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn runtime_gateway_json_csv_u64(row: &serde_json::Value, field: &str) -> String {
    row.get(field)
        .and_then(serde_json::Value::as_u64)
        .map(|value| value.to_string())
        .unwrap_or_default()
}

fn runtime_gateway_json_csv_f64(row: &serde_json::Value, field: &str) -> String {
    row.get(field)
        .and_then(serde_json::Value::as_f64)
        .map(|value| value.to_string())
        .unwrap_or_default()
}

fn runtime_gateway_csv_row<I, S>(csv: &mut String, values: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut first = true;
    for value in values {
        if first {
            first = false;
        } else {
            csv.push(',');
        }
        runtime_gateway_csv_cell(csv, value.as_ref());
    }
    csv.push('\n');
}

fn runtime_gateway_csv_cell(csv: &mut String, value: &str) {
    let formula_like = value
        .trim_start_matches([' ', '\t', '\r'])
        .starts_with(['=', '+', '-', '@']);
    let needs_quote = value
        .bytes()
        .any(|byte| matches!(byte, b',' | b'"' | b'\n' | b'\r'));
    if !needs_quote {
        if formula_like {
            csv.push('\'');
        }
        csv.push_str(value);
        return;
    }
    csv.push('"');
    if formula_like {
        csv.push('\'');
    }
    for ch in value.chars() {
        if ch == '"' {
            csv.push('"');
        }
        csv.push(ch);
    }
    csv.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct LedgerRecord {
        call_id: String,
        key_name: String,
        model: String,
        phase: String,
        request: u64,
        created_at_epoch: u64,
        minute_epoch: u64,
        input_tokens: u64,
        output_tokens: Option<u64>,
        response_status: Option<u16>,
    }

    #[test]
    fn ledger_csv_quotes_cells_that_need_escaping() {
        let csv = runtime_gateway_billing_ledger_csv(&[LedgerRecord {
            call_id: "prodex-1".to_string(),
            key_name: "team,\"a\"".to_string(),
            model: "gpt-5.4".to_string(),
            phase: "request".to_string(),
            request: 1,
            created_at_epoch: 2,
            minute_epoch: 3,
            input_tokens: 4,
            output_tokens: Some(5),
            response_status: Some(200),
        }]);
        assert!(csv.contains("prodex-1,\"team,\"\"a\"\"\",gpt-5.4"));
        assert!(csv.starts_with("call_id,key_name,model"));
    }

    #[test]
    fn ledger_csv_neutralizes_spreadsheet_formulas() {
        let csv = runtime_gateway_billing_ledger_csv(&[LedgerRecord {
            call_id: "prodex-1".to_string(),
            key_name: "team-a".to_string(),
            model: "=HYPERLINK(\"https://example.test\")".to_string(),
            phase: "request".to_string(),
            request: 1,
            created_at_epoch: 2,
            minute_epoch: 3,
            input_tokens: 4,
            output_tokens: Some(5),
            response_status: Some(200),
        }]);

        assert!(csv.contains("\"'=HYPERLINK(\"\"https://example.test\"\")\""));
    }

    #[test]
    fn summary_csv_includes_governance_dimensions() {
        let summary = serde_json::json!({
            "totals": {"requests": 1},
            "by_team": [{"team_id": "platform", "requests": 1}]
        });
        let csv = runtime_gateway_billing_summary_csv(&summary);
        assert!(csv.contains("group,key_name,model,team_id,project_id,user_id,budget_id"));
        assert!(csv.contains("by_team,,,platform,,,"));
    }
}
