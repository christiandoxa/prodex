use super::super::logging::{
    app_server_broker_audit_preview_summary, app_server_broker_log_preview_event,
    app_server_broker_log_preview_summary,
};
use super::super::parse::APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES;
use super::super::validation::PreviewSession;
use crate::initialize_runtime_proxy_log_path;
use serde_json::Value;
use std::io::{BufRead, Read, Write};

#[cfg(test)]
pub(crate) fn app_server_broker_write_stdio_preview_stream<R: BufRead, W: Write>(
    reader: R,
    writer: W,
) -> anyhow::Result<()> {
    write_preview_stream(reader, std::io::sink(), writer, "stdio-preview").map(|_| ())
}

pub(crate) fn app_server_broker_write_stdio_passthrough_preview_stream<
    R: BufRead,
    W: Write,
    D: Write,
>(
    reader: R,
    passthrough_writer: W,
    diagnostics_writer: D,
) -> anyhow::Result<()> {
    write_preview_stream(
        reader,
        passthrough_writer,
        diagnostics_writer,
        "stdio-passthrough-preview",
    )
    .map(|_| ())
}

fn write_preview_stream<R: BufRead, W: Write, D: Write>(
    mut reader: R,
    mut passthrough_writer: W,
    mut diagnostics_writer: D,
    mode: &'static str,
) -> anyhow::Result<Value> {
    let log_path = initialize_runtime_proxy_log_path();
    let mut session = PreviewSession::default();
    let mut raw_line = String::new();
    let mut line_index = 0usize;
    loop {
        raw_line.clear();
        let bytes_read = Read::by_ref(&mut reader)
            .take((APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES + 2) as u64)
            .read_line(&mut raw_line)?;
        if bytes_read == 0 {
            break;
        }
        if raw_line.len() > APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES && !raw_line.ends_with('\n') {
            anyhow::bail!(
                "app-server broker preview line exceeds {} bytes before newline",
                APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES
            );
        }
        line_index += 1;

        // Observation mode deliberately mirrors bytes before parsing or diagnostics.
        passthrough_writer.write_all(raw_line.as_bytes())?;
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let preview = session.observe_line(line_index, line);
        app_server_broker_log_preview_event(&log_path, line_index, &preview);
        serde_json::to_writer(&mut diagnostics_writer, &preview)?;
        diagnostics_writer.write_all(b"\n")?;
    }
    let summary = session.into_report_json();
    app_server_broker_log_preview_summary(&log_path, &summary);
    app_server_broker_audit_preview_summary(mode, &summary)?;
    serde_json::to_writer(&mut diagnostics_writer, &summary)?;
    diagnostics_writer.write_all(b"\n")?;
    Ok(summary)
}
