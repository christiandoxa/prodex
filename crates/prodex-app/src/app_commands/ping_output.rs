use super::PingResult;
use anyhow::Result;
use prodex_mojo_core::rich::{PingReportInput, PingReportKind, PingReportRow, render_ping_report};
use std::time::Duration;
use terminal_ui::print_stdout_line;

pub(super) fn render_ping_result(result: &PingResult) -> Result<()> {
    print_stdout_line(&ping_report_text(
        std::slice::from_ref(result),
        Duration::ZERO,
        PingReportKind::Profile,
    )?)?;
    Ok(())
}

pub(super) fn render_ping_summary(
    results: &[PingResult],
    elapsed: Duration,
    json: bool,
) -> Result<()> {
    let kind = if json {
        PingReportKind::SummaryJson
    } else {
        PingReportKind::SummaryHuman
    };
    print_stdout_line(&ping_report_text(results, elapsed, kind)?)?;
    Ok(())
}

pub(super) fn ping_report_text(
    results: &[PingResult],
    elapsed: Duration,
    kind: PingReportKind,
) -> Result<String> {
    let rows = results
        .iter()
        .map(|result| PingReportRow {
            profile: &result.profile,
            status: result.status.mojo_code(),
            model: result.model.as_deref(),
            effort: result.effort.as_deref(),
            effective_model: result.effective_model.as_deref(),
            detail: &result.detail,
            first_response_latency_ms: result.first_response_latency_ms,
            latency_ms: result.latency_ms,
        })
        .collect::<Vec<_>>();
    render_ping_report(PingReportInput {
        rows: &rows,
        kind,
        elapsed_ms: elapsed.as_millis(),
    })
    .map_err(|error| anyhow::anyhow!("Mojo ping report rendering failed: {error:?}"))
}

#[cfg(test)]
#[path = "ping_output/report_tests.rs"]
mod report_tests;
