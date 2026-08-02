use super::{capability_redacted_detail, optional_tool_health_status};

pub(super) fn collect_setup_optional_tool_health() -> Vec<prodex_optional_tools::ToolHealth> {
    prodex_optional_tools::OptionalToolSet::super_defaults()
        .iter()
        .map(prodex_optional_tools::optional_tool_status)
        .collect()
}

pub(super) fn setup_optional_tool_rows(
    tools: &[prodex_optional_tools::ToolHealth],
) -> Vec<(String, String)> {
    tools
        .iter()
        .map(|tool| {
            (
                tool.id.to_string(),
                format!(
                    "{}; {}",
                    optional_tool_health_status(tool),
                    capability_redacted_detail(&tool.detail)
                ),
            )
        })
        .collect()
}

pub(super) fn setup_optional_tool_verification_json(
    caveman: Option<&prodex_optional_tools::ToolHealth>,
    tools: Option<&[prodex_optional_tools::ToolHealth]>,
    requested: bool,
    dry_run: bool,
) -> serde_json::Value {
    let tool_rows = tools.map_or_else(
        || {
            if requested && dry_run {
                prodex_optional_tools::OptionalToolSet::super_defaults()
                    .iter()
                    .map(|id| {
                        serde_json::json!({
                            "id": id.as_str(),
                            "status": "not checked (dry-run)",
                            "detail": "verification skipped during dry-run",
                        })
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        },
        |tools| {
            tools
                .iter()
                .map(|tool| {
                    serde_json::json!({
                        "id": tool.id.as_str(),
                        "status": optional_tool_health_status(tool),
                        "detail": capability_redacted_detail(&tool.detail),
                    })
                })
                .collect()
        },
    );
    let status = caveman
        .map(optional_tool_health_status)
        .unwrap_or_else(|| "not checked (dry-run)".to_string());
    let detail = caveman
        .map(|health| capability_redacted_detail(&health.detail))
        .unwrap_or_else(|| "verification skipped during dry-run".to_string());
    serde_json::json!({
        "id": "caveman",
        "status": status,
        "detail": detail,
        "requested": requested,
        "ready": tools.map(|tools| tools.iter().all(|tool| {
            tool.status == prodex_optional_tools::ToolHealthStatus::Installed
        })),
        "tools": tool_rows,
    })
}

pub(super) fn ensure_optional_tools_installed(
    tools: &[prodex_optional_tools::ToolHealth],
) -> anyhow::Result<()> {
    for tool in tools {
        super::ensure_optional_tool_installed(tool)?;
    }
    Ok(())
}
