use super::super::logging::{
    app_server_broker_audit_preview_summary, app_server_broker_log_preview_event,
    app_server_broker_log_preview_summary,
};
use super::super::parse::APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES;
use super::super::validation::PreviewSession;
use super::validate::validation_failure_counts;
use crate::initialize_runtime_proxy_log_path;
use std::io::{BufRead, Read, Write};

pub(crate) fn app_server_broker_write_stdio_validate_passthrough_stream<
    R: BufRead,
    W: Write,
    D: Write,
>(
    reader: R,
    passthrough_writer: W,
    diagnostics_writer: D,
) -> anyhow::Result<()> {
    write_validate_passthrough_stream(
        reader,
        passthrough_writer,
        diagnostics_writer,
        "stdio-validate-passthrough",
    )
}

pub(super) fn write_validate_passthrough_stream<R: BufRead, W: Write, D: Write>(
    mut reader: R,
    mut passthrough_writer: W,
    mut diagnostics_writer: D,
    mode: &'static str,
) -> anyhow::Result<()> {
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
        let line = raw_line.trim();
        if line.is_empty() {
            passthrough_writer.write_all(raw_line.as_bytes())?;
            passthrough_writer.flush()?;
            continue;
        }
        let observation = session.validate_line(line_index, line);
        app_server_broker_log_preview_event(&log_path, line_index, &observation.preview);
        serde_json::to_writer(&mut diagnostics_writer, &observation.preview)?;
        diagnostics_writer.write_all(b"\n")?;
        diagnostics_writer.flush()?;

        let parse_failed = !observation.preview["preview"]["parse_ok"]
            .as_bool()
            .unwrap_or_default();
        let invalid_frame = observation.preview["preview"]["summary"]["frame_kind"]
            .as_str()
            .is_some_and(|frame_kind| frame_kind == "invalid");
        if parse_failed || invalid_frame {
            let summary = session.into_report_json();
            app_server_broker_log_preview_summary(&log_path, &summary);
            app_server_broker_audit_preview_summary(mode, &summary);
            serde_json::to_writer(&mut diagnostics_writer, &summary)?;
            diagnostics_writer.write_all(b"\n")?;
            let (parse_error_count, invalid_frame_count) = validation_failure_counts(&summary);
            anyhow::bail!(
                "app-server broker validation failed before passthrough: parse_error_count={parse_error_count} invalid_frame_count={invalid_frame_count}"
            );
        }
        if let Some(failure) = observation.lifecycle_failure {
            finish_failed_session(session, &log_path, mode, &mut diagnostics_writer)?;
            anyhow::bail!(
                "app-server broker lifecycle validation failed before passthrough: {failure}"
            );
        }
        if let Some(failure) = observation.request_response_failure {
            finish_failed_session(session, &log_path, mode, &mut diagnostics_writer)?;
            anyhow::bail!(
                "app-server broker request/response validation failed before passthrough: {failure}"
            );
        }
        if let Some(failure) = observation.lifecycle_payload_failure {
            finish_failed_session(session, &log_path, mode, &mut diagnostics_writer)?;
            anyhow::bail!(
                "app-server broker lifecycle payload validation failed before passthrough: {failure}"
            );
        }

        // Validate-before-forward ordering is intentionally explicit.
        passthrough_writer.write_all(raw_line.as_bytes())?;
        passthrough_writer.flush()?;
    }
    let pending_failure = session.finish(line_index);
    let summary = session.into_report_json();
    app_server_broker_log_preview_summary(&log_path, &summary);
    app_server_broker_audit_preview_summary(mode, &summary);
    serde_json::to_writer(&mut diagnostics_writer, &summary)?;
    diagnostics_writer.write_all(b"\n")?;
    diagnostics_writer.flush()?;
    if let Some(failure) = pending_failure {
        anyhow::bail!("app-server broker request/response validation failed at EOF: {failure}");
    }
    Ok(())
}

fn finish_failed_session<D: Write>(
    session: PreviewSession,
    log_path: &std::path::Path,
    mode: &'static str,
    diagnostics_writer: &mut D,
) -> anyhow::Result<()> {
    let summary = session.into_report_json();
    app_server_broker_log_preview_summary(log_path, &summary);
    app_server_broker_audit_preview_summary(mode, &summary);
    serde_json::to_writer(&mut *diagnostics_writer, &summary)?;
    diagnostics_writer.write_all(b"\n")?;
    Ok(())
}
