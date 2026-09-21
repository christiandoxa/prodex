use super::{LogStreamItem, OutputThroughput, operational_event_from_runtime_line};
use crate::reports::info_token_usage_progress_event_from_line;
use std::path::Path;
use std::time::Instant;

pub(super) fn log_items(
    line: &str,
    include_operational_insights: bool,
    coalesce_load: bool,
) -> anyhow::Result<Vec<LogStreamItem>> {
    if !include_operational_insights {
        return Ok(Vec::new());
    }
    let Some(event) = operational_event_from_runtime_line(line)? else {
        return Ok(Vec::new());
    };
    if coalesce_load {
        return Ok(vec![match event.load {
            Some(load) => LogStreamItem::LoadObservation(load),
            None => LogStreamItem::Transcript(event.transcript),
        }]);
    }
    Ok(vec![LogStreamItem::Transcript(event.transcript)])
}

pub(super) fn observe_token_usage_progress(
    path: &Path,
    line: &str,
    throughput: Option<&mut OutputThroughput>,
) {
    if let Some(event) = info_token_usage_progress_event_from_line(line)
        && let Some(throughput) = throughput
    {
        throughput.observe_token_usage(path, &event, Instant::now());
    }
}
