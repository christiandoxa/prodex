use super::frame::AllQuotaWatchTuiRow;
use super::*;
use prodex_mojo_core::json::QuotaWatchPresentation;

fn quota_watch_projection<T: serde::de::DeserializeOwned>(
    facts: &serde_json::Value,
    operation: QuotaWatchPresentation,
    detail: bool,
) -> T {
    let tree = crate::json_boundary::app_json_tree(facts)
        .expect("bounded quota presentation facts must fit the JSON ABI");
    let raw = std::str::from_utf8(&tree.raw).expect("Serde quota facts remain UTF-8");
    let output =
        prodex_mojo_core::json::quota_watch_presentation(&tree.nodes, raw, operation, detail)
            .expect("compiled Mojo quota presentation returned invalid output");
    serde_json::from_slice(&output).expect("Mojo quota presentation matches its caller schema")
}

// Existing provider renderers supply observations; Mojo owns display choices and assembly.
fn quota_watch_snapshot_facts(snapshot: &ProviderQuotaSnapshot) -> serde_json::Value {
    match snapshot {
        ProviderQuotaSnapshot::OpenAi(usage) => serde_json::json!({
            "kind": "openai",
            "account": usage.email,
            "plan": usage.plan_type,
            "main": format_main_windows(usage),
            "compact_main": format_main_windows_compact(usage),
            "status": format_openai_quota_status(usage),
            "reset": prodex_quota::format_main_reset_summary(usage),
            "reset_credits": usage.rate_limit_reset_credits.as_ref().map(|credits| credits.available_count),
            "additional": prodex_quota::format_openai_additional_limit_summaries(usage),
        }),
        ProviderQuotaSnapshot::Copilot(info) => serde_json::json!({
            "kind": "copilot",
            "account": info.login,
            "plan": info.copilot_plan,
            "fallback_plan": info.access_type_sku,
            "main": format_copilot_main_quota(info),
            "status": format_copilot_quota_status(info),
            "reset": format_copilot_reset_summary(info),
        }),
        ProviderQuotaSnapshot::Gemini(info) => serde_json::json!({
            "kind": "gemini",
            "account": info.email,
            "plan": info.plan,
            "main": format_gemini_main_quota(info),
            "status": format_gemini_quota_status(info),
            "reset": format_gemini_reset_summary(info),
        }),
        ProviderQuotaSnapshot::External(info) => serde_json::json!({
            "kind": "external",
            "account": info.account,
            "plan": info.plan,
            "main": info.main,
            "status": info.status,
            "reset": info.reset,
        }),
    }
}

pub(super) fn build_all_quota_watch_tui_row(
    report: &QuotaReport,
    detail: bool,
) -> AllQuotaWatchTuiRow {
    let mut facts = match &report.result {
        Ok(snapshot) => quota_watch_snapshot_facts(snapshot),
        Err(error) => serde_json::json!({
            "kind": "error",
            "status": format_quota_error_status(error),
            "reset": prodex_quota::format_quota_error_detail(error),
        }),
    };
    facts["profile"] = serde_json::json!(report.name);
    facts["active"] = serde_json::json!(report.active);
    facts["auth"] = serde_json::json!(report.auth.label);
    facts["workspace_name"] = serde_json::json!(report.workspace_name);
    facts["workspace_id"] = serde_json::json!(report.workspace_id);
    quota_watch_projection(&facts, QuotaWatchPresentation::Row, detail)
}

pub(super) fn quota_human_tui_has_compact_label(line: &str) -> bool {
    quota_watch_projection(
        &serde_json::json!(line),
        QuotaWatchPresentation::CompactLabel,
        false,
    )
}

pub(super) fn quota_watch_profile_fields(
    updated: &str,
    snapshot: &ProviderQuotaSnapshot,
    detail: bool,
) -> Vec<(String, String)> {
    let mut facts = quota_watch_snapshot_facts(snapshot);
    facts["updated"] = serde_json::json!(updated);
    quota_watch_projection(&facts, QuotaWatchPresentation::ProfileFields, detail)
}

#[cfg(test)]
#[path = "frame_data_tests.rs"]
mod tests;
