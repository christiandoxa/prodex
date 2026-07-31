use super::super::logging::{
    app_server_broker_audit_preview_summary, app_server_broker_log_preview_event,
    app_server_broker_log_preview_summary,
};
use super::super::parse::APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES;
use super::super::validation::{PreviewObservation, PreviewSession, ValidationFailure};
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
    let log_path = initialize_runtime_proxy_log_path()?;
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
        if let Err(failure) = write_preview_observations(
            &mut session,
            line_index,
            line,
            &log_path,
            &mut diagnostics_writer,
        )? {
            return finish_passthrough_failure(
                session,
                &log_path,
                mode,
                &mut diagnostics_writer,
                failure,
            );
        }

        // Validate-before-forward ordering is intentionally explicit.
        passthrough_writer.write_all(raw_line.as_bytes())?;
        passthrough_writer.flush()?;
    }
    let pending_failure = session.finish(line_index);
    let summary = session.into_report_json();
    app_server_broker_log_preview_summary(&log_path, &summary);
    app_server_broker_audit_preview_summary(mode, &summary)?;
    serde_json::to_writer(&mut diagnostics_writer, &summary)?;
    diagnostics_writer.write_all(b"\n")?;
    diagnostics_writer.flush()?;
    if let Some(failure) = pending_failure {
        anyhow::bail!("app-server broker request/response validation failed at EOF: {failure}");
    }
    Ok(())
}

enum PassthroughFailure {
    InvalidFrame,
    Validation { kind: &'static str, failure: String },
}

fn write_preview_observations<D: Write>(
    session: &mut PreviewSession,
    line_index: usize,
    line: &str,
    log_path: &std::path::Path,
    diagnostics_writer: &mut D,
) -> anyhow::Result<Result<(), PassthroughFailure>> {
    for observation in session.validate_line(line_index, line) {
        write_preview_observation(line_index, log_path, diagnostics_writer, &observation)?;
        if preview_is_invalid(&observation) {
            return Ok(Err(PassthroughFailure::InvalidFrame));
        }
        if let Some((kind, failure)) = observation_validation_failure(&observation) {
            return Ok(Err(PassthroughFailure::Validation {
                kind,
                failure: failure.to_string(),
            }));
        }
    }
    Ok(Ok(()))
}

fn write_preview_observation<D: Write>(
    line_index: usize,
    log_path: &std::path::Path,
    diagnostics_writer: &mut D,
    observation: &PreviewObservation,
) -> anyhow::Result<()> {
    app_server_broker_log_preview_event(log_path, line_index, &observation.preview);
    serde_json::to_writer(&mut *diagnostics_writer, &observation.preview)?;
    diagnostics_writer.write_all(b"\n")?;
    diagnostics_writer.flush()?;
    Ok(())
}

fn preview_is_invalid(observation: &PreviewObservation) -> bool {
    !observation.preview["preview"]["parse_ok"]
        .as_bool()
        .unwrap_or_default()
        || observation.preview["preview"]["summary"]["frame_kind"]
            .as_str()
            .is_some_and(|frame_kind| frame_kind == "invalid")
}

fn observation_validation_failure(
    observation: &PreviewObservation,
) -> Option<(&'static str, &ValidationFailure)> {
    observation
        .lifecycle_failure
        .as_ref()
        .map(|failure| ("lifecycle", failure))
        .or_else(|| {
            observation
                .request_response_failure
                .as_ref()
                .map(|failure| ("request/response", failure))
        })
        .or_else(|| {
            observation
                .lifecycle_payload_failure
                .as_ref()
                .map(|failure| ("lifecycle payload", failure))
        })
}

fn finish_passthrough_failure<D: Write>(
    session: PreviewSession,
    log_path: &std::path::Path,
    mode: &'static str,
    diagnostics_writer: &mut D,
    failure: PassthroughFailure,
) -> anyhow::Result<()> {
    match failure {
        PassthroughFailure::InvalidFrame => {
            let summary = session.into_report_json();
            app_server_broker_log_preview_summary(log_path, &summary);
            app_server_broker_audit_preview_summary(mode, &summary)?;
            serde_json::to_writer(&mut *diagnostics_writer, &summary)?;
            diagnostics_writer.write_all(b"\n")?;
            let (parse_error_count, invalid_frame_count) = validation_failure_counts(&summary);
            anyhow::bail!(
                "app-server broker validation failed before passthrough: parse_error_count={parse_error_count} invalid_frame_count={invalid_frame_count}"
            );
        }
        PassthroughFailure::Validation { kind, failure } => {
            finish_failed_session(session, log_path, mode, diagnostics_writer)?;
            anyhow::bail!(
                "app-server broker {kind} validation failed before passthrough: {failure}"
            );
        }
    }
}

fn finish_failed_session<D: Write>(
    session: PreviewSession,
    log_path: &std::path::Path,
    mode: &'static str,
    diagnostics_writer: &mut D,
) -> anyhow::Result<()> {
    let summary = session.into_report_json();
    app_server_broker_log_preview_summary(log_path, &summary);
    app_server_broker_audit_preview_summary(mode, &summary)?;
    serde_json::to_writer(&mut *diagnostics_writer, &summary)?;
    diagnostics_writer.write_all(b"\n")?;
    Ok(())
}
