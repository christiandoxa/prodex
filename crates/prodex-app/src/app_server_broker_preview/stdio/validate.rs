use super::super::logging::{
    app_server_broker_audit_preview_summary, app_server_broker_log_preview_event,
    app_server_broker_log_preview_summary,
};
use super::super::parse::APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES;
use super::super::validation::{PreviewSession, ValidationFailure};
use crate::initialize_runtime_proxy_log_path;
use serde_json::Value;
use std::io::{BufRead, Read, Write};

type ValidationSummary = (
    Value,
    Option<ValidationFailure>,
    Option<ValidationFailure>,
    Option<ValidationFailure>,
);

pub(crate) fn app_server_broker_write_stdio_validate_stream<R: BufRead, W: Write>(
    reader: R,
    writer: W,
) -> anyhow::Result<()> {
    let (summary, lifecycle_failure, request_response_failure, lifecycle_payload_failure) =
        write_validate_diagnostic_stream(reader, writer, "stdio-validate")?;
    ensure_valid_summary(
        &summary,
        lifecycle_failure,
        request_response_failure,
        lifecycle_payload_failure,
    )
}

pub(crate) fn app_server_broker_write_stdio_live_stream<R: BufRead, W: Write, D: Write>(
    reader: R,
    mut passthrough_writer: W,
    diagnostics_writer: D,
) -> anyhow::Result<()> {
    let mut passthrough_buffer = Vec::new();
    let (summary, lifecycle_failure, request_response_failure, lifecycle_payload_failure) =
        write_live_validate_stream(reader, &mut passthrough_buffer, diagnostics_writer)?;
    ensure_valid_summary(
        &summary,
        lifecycle_failure,
        request_response_failure,
        lifecycle_payload_failure,
    )?;

    // Live mode writes only after the complete session passes validation.
    passthrough_writer.write_all(&passthrough_buffer)?;
    Ok(())
}

fn ensure_valid_summary(
    summary: &Value,
    lifecycle_failure: Option<ValidationFailure>,
    request_response_failure: Option<ValidationFailure>,
    lifecycle_payload_failure: Option<ValidationFailure>,
) -> anyhow::Result<()> {
    let (parse_error_count, invalid_frame_count) = validation_failure_counts(summary);
    if parse_error_count > 0 || invalid_frame_count > 0 {
        anyhow::bail!(
            "app-server broker validation failed: parse_error_count={parse_error_count} invalid_frame_count={invalid_frame_count}"
        );
    }
    if let Some(failure) = lifecycle_failure {
        anyhow::bail!("app-server broker lifecycle validation failed: {failure}");
    }
    if let Some(failure) = request_response_failure {
        anyhow::bail!("app-server broker request/response validation failed: {failure}");
    }
    if let Some(failure) = lifecycle_payload_failure {
        anyhow::bail!("app-server broker lifecycle payload validation failed: {failure}");
    }
    Ok(())
}

fn write_validate_diagnostic_stream<R: BufRead, W: Write>(
    mut reader: R,
    mut diagnostics_writer: W,
    mode: &'static str,
) -> anyhow::Result<ValidationSummary> {
    let log_path = initialize_runtime_proxy_log_path();
    let mut session = PreviewSession::default();
    let mut lifecycle_failure = None;
    let mut request_response_failure = None;
    let mut lifecycle_payload_failure = None;
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
            continue;
        }
        let observation = session.validate_line(line_index, line);
        app_server_broker_log_preview_event(&log_path, line_index, &observation.preview);
        serde_json::to_writer(&mut diagnostics_writer, &observation.preview)?;
        diagnostics_writer.write_all(b"\n")?;
        lifecycle_failure = lifecycle_failure.or(observation.lifecycle_failure);
        request_response_failure =
            request_response_failure.or(observation.request_response_failure);
        lifecycle_payload_failure =
            lifecycle_payload_failure.or(observation.lifecycle_payload_failure);
    }
    if lifecycle_failure.is_none()
        && request_response_failure.is_none()
        && lifecycle_payload_failure.is_none()
    {
        request_response_failure = session.finish(line_index);
    }
    let summary = session.into_report_json();
    app_server_broker_log_preview_summary(&log_path, &summary);
    app_server_broker_audit_preview_summary(mode, &summary);
    serde_json::to_writer(&mut diagnostics_writer, &summary)?;
    diagnostics_writer.write_all(b"\n")?;
    Ok((
        summary,
        lifecycle_failure,
        request_response_failure,
        lifecycle_payload_failure,
    ))
}

fn write_live_validate_stream<R: BufRead, D: Write>(
    mut reader: R,
    passthrough_buffer: &mut Vec<u8>,
    mut diagnostics_writer: D,
) -> anyhow::Result<ValidationSummary> {
    let log_path = initialize_runtime_proxy_log_path();
    let mut session = PreviewSession::default();
    let mut lifecycle_failure = None;
    let mut request_response_failure = None;
    let mut lifecycle_payload_failure = None;
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

        // Live mode preserves the original byte order in its private buffer.
        passthrough_buffer.extend_from_slice(raw_line.as_bytes());
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let observation = session.validate_line(line_index, line);
        app_server_broker_log_preview_event(&log_path, line_index, &observation.preview);
        serde_json::to_writer(&mut diagnostics_writer, &observation.preview)?;
        diagnostics_writer.write_all(b"\n")?;
        lifecycle_failure = lifecycle_failure.or(observation.lifecycle_failure);
        request_response_failure =
            request_response_failure.or(observation.request_response_failure);
        lifecycle_payload_failure =
            lifecycle_payload_failure.or(observation.lifecycle_payload_failure);
    }
    if lifecycle_failure.is_none()
        && request_response_failure.is_none()
        && lifecycle_payload_failure.is_none()
    {
        request_response_failure = session.finish(line_index);
    }
    let summary = session.into_report_json();
    app_server_broker_log_preview_summary(&log_path, &summary);
    app_server_broker_audit_preview_summary("stdio-live", &summary);
    serde_json::to_writer(&mut diagnostics_writer, &summary)?;
    diagnostics_writer.write_all(b"\n")?;
    Ok((
        summary,
        lifecycle_failure,
        request_response_failure,
        lifecycle_payload_failure,
    ))
}

pub(super) fn validation_failure_counts(summary: &Value) -> (u64, u64) {
    let report = &summary["report"];
    let parse_error_count = report["error_count"].as_u64().unwrap_or_default();
    let invalid_frame_count = report["frame_kind_counts"]["invalid"]
        .as_u64()
        .unwrap_or_default();
    (parse_error_count, invalid_frame_count)
}
