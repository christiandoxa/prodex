use crate::reports::InfoTokenUsageEvent;
use anyhow::{Context, Result};
use prodex_mojo_core::json::{LogTokenPresentation, log_token_presentation_json};
use serde::de::DeserializeOwned;

#[derive(serde::Deserialize)]
pub(in crate::app_commands::log) enum TokenLogTone {
    Raw,
    Muted,
    Token,
    Primary,
    Metric,
    Accent,
    Tool,
}

pub(in crate::app_commands::log) fn project_token_log<T: DeserializeOwned>(
    event: &InfoTokenUsageEvent,
    operation: LogTokenPresentation,
) -> Result<T> {
    let rate = super::format_output_tokens_per_second;
    let input = serde_json::to_value((
        event,
        event
            .output_tokens_per_second
            .map(|value| rate(Some(value))),
        rate(None),
    ))?;
    let tree = crate::json_boundary::app_json_tree(&input)?;
    let raw = std::str::from_utf8(&tree.raw).context("invalid token log JSON encoding")?;
    let output = log_token_presentation_json(&tree.nodes, raw, operation)
        .map_err(|error| anyhow::anyhow!("Mojo token log projection failed: {error:?}"))?;
    serde_json::from_slice(&output).context("invalid compiled token log projection")
}
