#[path = "app_server_broker_preview/contract.rs"]
mod contract;
#[path = "app_server_broker_preview/logging.rs"]
mod logging;
#[path = "app_server_broker_preview/parse.rs"]
mod parse;
#[path = "app_server_broker_preview/report.rs"]
mod report;
#[path = "app_server_broker_preview/stdio/mod.rs"]
mod stdio;
#[path = "app_server_broker_preview/validation/mod.rs"]
mod validation;

pub(crate) use contract::app_server_broker_render_output;
#[cfg(test)]
pub(crate) use contract::{app_server_broker_contract_json, app_server_broker_status_line};
#[cfg(test)]
pub(crate) use parse::{APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES, app_server_broker_preview_line};
pub(crate) use stdio::{
    app_server_broker_write_stdio_live_stream,
    app_server_broker_write_stdio_passthrough_preview_stream,
    app_server_broker_write_stdio_validate_passthrough_stream,
    app_server_broker_write_stdio_validate_stream,
};

#[cfg(test)]
pub(crate) use stdio::app_server_broker_write_stdio_preview_stream;

#[cfg(test)]
use report::app_server_broker_preview_report_from_previews;
#[cfg(test)]
use serde_json::Value;
#[cfg(test)]
use validation::PreviewSession;

#[cfg(test)]
pub(crate) fn app_server_broker_preview_lines(input: &str) -> Vec<Value> {
    let mut session = PreviewSession::default();
    for (index, line) in input.lines().enumerate() {
        let line = line.trim();
        if !line.is_empty() {
            session.observe_line(index + 1, line);
        }
    }
    session.into_previews()
}

#[cfg(test)]
pub(crate) fn app_server_broker_preview_report(input: &str) -> Value {
    app_server_broker_preview_report_from_previews(app_server_broker_preview_lines(input))
}

#[cfg(test)]
pub(crate) fn app_server_broker_preview_report_json(input: &str) -> Value {
    serde_json::json!({
        "object": "app_server_broker.preview_report",
        "report": app_server_broker_preview_report(input),
    })
}

#[cfg(test)]
pub(crate) fn app_server_broker_render_stdio_preview(input: &str) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(
        &app_server_broker_preview_report_json(input),
    )?)
}
