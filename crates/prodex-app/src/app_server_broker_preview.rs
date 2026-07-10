#[path = "app_server_broker_preview/logging.rs"]
mod logging;
#[path = "app_server_broker_preview/report.rs"]
mod report;

use self::logging::{
    app_server_broker_audit_preview_summary,
    app_server_broker_audit_preview_summary_required_fields, app_server_broker_log_preview_event,
    app_server_broker_log_preview_summary,
};
use self::report::app_server_broker_preview_report_from_previews;
use super::app_server_broker_protocol::{
    app_server_broker_commit_boundaries, app_server_broker_continuation_decision_kinds,
    app_server_broker_diagnostic_summary_json, app_server_broker_lifecycle_methods,
    app_server_broker_lifecycle_response_schema_file, app_server_broker_policy_modes,
    app_server_broker_rotation_windows, app_server_broker_routing_hints,
};
use crate::initialize_runtime_proxy_log_path;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, Read, Write};

pub(crate) const APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES: usize = 1024 * 1024;

pub(crate) fn app_server_broker_preview_line(line: &str) -> Value {
    if line.len() > APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES {
        return serde_json::json!({
            "parse_ok": false,
            "error": "line_too_large",
            "message": format!(
                "app-server broker preview line exceeds {} bytes",
                APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES
            ),
        });
    }

    match serde_json::from_str::<Value>(line) {
        Ok(value) => serde_json::json!({
            "parse_ok": true,
            "summary": app_server_broker_diagnostic_summary_json(&value),
        }),
        Err(error) => serde_json::json!({
            "parse_ok": false,
            "error": "invalid_json",
            "message": error.to_string(),
        }),
    }
}

pub(crate) fn app_server_broker_preview_lines(input: &str) -> Vec<Value> {
    let mut request_response = AppServerBrokerRequestResponseValidation::default();
    let mut previews = Vec::new();
    for (index, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut preview = serde_json::json!({
            "object": "app_server_broker.preview_event",
            "line": index + 1,
            "preview": app_server_broker_preview_line(line),
        });
        let frame = serde_json::from_str::<Value>(line).ok();
        request_response.annotate_response_schema(&mut preview, frame.as_ref());
        request_response.observe_for_schema_tracking(&preview);
        previews.push(preview);
    }
    previews
}

pub(crate) fn app_server_broker_preview_report(input: &str) -> Value {
    app_server_broker_preview_report_from_previews(app_server_broker_preview_lines(input))
}

pub(crate) fn app_server_broker_preview_report_json(input: &str) -> Value {
    app_server_broker_preview_report_json_from_previews(app_server_broker_preview_lines(input))
}

pub(crate) fn app_server_broker_write_stdio_preview_stream<R: BufRead, W: Write>(
    reader: R,
    writer: W,
) -> anyhow::Result<()> {
    app_server_broker_write_stdio_preview_stream_with_mode(
        reader,
        std::io::sink(),
        writer,
        "stdio-preview",
    )
    .map(|_| ())
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
    app_server_broker_write_stdio_preview_stream_with_mode(
        reader,
        passthrough_writer,
        diagnostics_writer,
        "stdio-passthrough-preview",
    )
    .map(|_| ())
}

pub(crate) fn app_server_broker_write_stdio_validate_stream<R: BufRead, W: Write>(
    reader: R,
    writer: W,
) -> anyhow::Result<()> {
    let (summary, lifecycle_failure, request_response_failure, lifecycle_payload_failure) =
        app_server_broker_write_stdio_validate_diagnostic_stream(reader, writer)?;
    let (parse_error_count, invalid_frame_count) =
        app_server_broker_validation_failure_counts(&summary);
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

fn app_server_broker_write_stdio_validate_diagnostic_stream<R: BufRead, W: Write>(
    mut reader: R,
    mut diagnostics_writer: W,
) -> anyhow::Result<(Value, Option<String>, Option<String>, Option<String>)> {
    let log_path = initialize_runtime_proxy_log_path();
    let mut previews = Vec::new();
    let mut lifecycle = AppServerBrokerLifecycleValidation::default();
    let mut request_response = AppServerBrokerRequestResponseValidation::default();
    let mut lifecycle_payload = AppServerBrokerLifecyclePayloadValidation;
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
        let mut preview = serde_json::json!({
            "object": "app_server_broker.preview_event",
            "line": line_index,
            "preview": app_server_broker_preview_line(line),
        });
        let frame = serde_json::from_str::<Value>(line).ok();
        request_response.annotate_response_schema(&mut preview, frame.as_ref());
        app_server_broker_log_preview_event(&log_path, line_index, &preview);
        serde_json::to_writer(&mut diagnostics_writer, &preview)?;
        diagnostics_writer.write_all(b"\n")?;
        if lifecycle_failure.is_none() {
            lifecycle_failure = lifecycle.observe_preview(&preview);
        }
        if request_response_failure.is_none() {
            request_response_failure =
                request_response.observe_preview_and_frame(&preview, frame.as_ref());
        }
        if lifecycle_payload_failure.is_none() {
            lifecycle_payload_failure =
                lifecycle_payload.observe_preview_and_frame(&preview, frame.as_ref());
        }
        previews.push(preview);
    }
    if lifecycle_failure.is_none()
        && request_response_failure.is_none()
        && lifecycle_payload_failure.is_none()
    {
        request_response_failure = request_response.finish(line_index);
    }
    let summary = app_server_broker_preview_report_json_from_previews(previews);
    app_server_broker_log_preview_summary(&log_path, &summary);
    app_server_broker_audit_preview_summary("stdio-validate", &summary);
    serde_json::to_writer(&mut diagnostics_writer, &summary)?;
    diagnostics_writer.write_all(b"\n")?;
    Ok((
        summary,
        lifecycle_failure,
        request_response_failure,
        lifecycle_payload_failure,
    ))
}

pub(crate) fn app_server_broker_write_stdio_validate_passthrough_stream<
    R: BufRead,
    W: Write,
    D: Write,
>(
    mut reader: R,
    mut passthrough_writer: W,
    mut diagnostics_writer: D,
) -> anyhow::Result<()> {
    app_server_broker_write_stdio_validate_passthrough_stream_with_mode(
        reader,
        passthrough_writer,
        diagnostics_writer,
        "stdio-validate-passthrough",
    )
}

pub(crate) fn app_server_broker_write_stdio_live_stream<R: BufRead, W: Write, D: Write>(
    reader: R,
    mut passthrough_writer: W,
    diagnostics_writer: D,
) -> anyhow::Result<()> {
    let mut passthrough_buffer = Vec::new();
    let (summary, lifecycle_failure, request_response_failure, lifecycle_payload_failure) =
        app_server_broker_write_stdio_live_validate_stream(
            reader,
            &mut passthrough_buffer,
            diagnostics_writer,
        )?;
    let (parse_error_count, invalid_frame_count) =
        app_server_broker_validation_failure_counts(&summary);
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
    passthrough_writer.write_all(&passthrough_buffer)?;
    Ok(())
}

fn app_server_broker_write_stdio_live_validate_stream<R: BufRead, D: Write>(
    mut reader: R,
    passthrough_buffer: &mut Vec<u8>,
    mut diagnostics_writer: D,
) -> anyhow::Result<(Value, Option<String>, Option<String>, Option<String>)> {
    let log_path = initialize_runtime_proxy_log_path();
    let mut previews = Vec::new();
    let mut lifecycle = AppServerBrokerLifecycleValidation::default();
    let mut request_response = AppServerBrokerRequestResponseValidation::default();
    let mut lifecycle_payload = AppServerBrokerLifecyclePayloadValidation;
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
        passthrough_buffer.extend_from_slice(raw_line.as_bytes());
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let mut preview = serde_json::json!({
            "object": "app_server_broker.preview_event",
            "line": line_index,
            "preview": app_server_broker_preview_line(line),
        });
        let frame = serde_json::from_str::<Value>(line).ok();
        request_response.annotate_response_schema(&mut preview, frame.as_ref());
        app_server_broker_log_preview_event(&log_path, line_index, &preview);
        serde_json::to_writer(&mut diagnostics_writer, &preview)?;
        diagnostics_writer.write_all(b"\n")?;
        if lifecycle_failure.is_none() {
            lifecycle_failure = lifecycle.observe_preview(&preview);
        }
        if request_response_failure.is_none() {
            request_response_failure =
                request_response.observe_preview_and_frame(&preview, frame.as_ref());
        }
        if lifecycle_payload_failure.is_none() {
            lifecycle_payload_failure =
                lifecycle_payload.observe_preview_and_frame(&preview, frame.as_ref());
        }
        previews.push(preview);
    }
    if lifecycle_failure.is_none()
        && request_response_failure.is_none()
        && lifecycle_payload_failure.is_none()
    {
        request_response_failure = request_response.finish(line_index);
    }
    let summary = app_server_broker_preview_report_json_from_previews(previews);
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

fn app_server_broker_write_stdio_validate_passthrough_stream_with_mode<
    R: BufRead,
    W: Write,
    D: Write,
>(
    mut reader: R,
    mut passthrough_writer: W,
    mut diagnostics_writer: D,
    mode: &'static str,
) -> anyhow::Result<()> {
    let log_path = initialize_runtime_proxy_log_path();
    let mut previews = Vec::new();
    let mut lifecycle = AppServerBrokerLifecycleValidation::default();
    let mut request_response = AppServerBrokerRequestResponseValidation::default();
    let mut lifecycle_payload = AppServerBrokerLifecyclePayloadValidation;
    let mut parse_error_count = 0u64;
    let mut invalid_frame_count = 0u64;
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
            continue;
        }
        let mut preview = serde_json::json!({
            "object": "app_server_broker.preview_event",
            "line": line_index,
            "preview": app_server_broker_preview_line(line),
        });
        let frame = serde_json::from_str::<Value>(line).ok();
        request_response.annotate_response_schema(&mut preview, frame.as_ref());
        app_server_broker_log_preview_event(&log_path, line_index, &preview);
        serde_json::to_writer(&mut diagnostics_writer, &preview)?;
        diagnostics_writer.write_all(b"\n")?;
        let lifecycle_failure = lifecycle.observe_preview(&preview);
        let request_response_failure =
            request_response.observe_preview_and_frame(&preview, frame.as_ref());
        let lifecycle_payload_failure =
            lifecycle_payload.observe_preview_and_frame(&preview, frame.as_ref());
        if !preview["preview"]["parse_ok"].as_bool().unwrap_or_default() {
            parse_error_count += 1;
        }
        if preview["preview"]["summary"]["frame_kind"]
            .as_str()
            .is_some_and(|frame_kind| frame_kind == "invalid")
        {
            invalid_frame_count += 1;
        }
        previews.push(preview);
        if parse_error_count > 0 || invalid_frame_count > 0 {
            let summary = app_server_broker_preview_report_json_from_previews(previews);
            app_server_broker_log_preview_summary(&log_path, &summary);
            app_server_broker_audit_preview_summary(mode, &summary);
            serde_json::to_writer(&mut diagnostics_writer, &summary)?;
            diagnostics_writer.write_all(b"\n")?;
            anyhow::bail!(
                "app-server broker validation failed before passthrough: parse_error_count={parse_error_count} invalid_frame_count={invalid_frame_count}"
            );
        }
        if lifecycle_failure.is_some()
            || request_response_failure.is_some()
            || lifecycle_payload_failure.is_some()
        {
            let summary = app_server_broker_preview_report_json_from_previews(previews);
            app_server_broker_log_preview_summary(&log_path, &summary);
            app_server_broker_audit_preview_summary(mode, &summary);
            serde_json::to_writer(&mut diagnostics_writer, &summary)?;
            diagnostics_writer.write_all(b"\n")?;
            if let Some(failure) = lifecycle_failure {
                anyhow::bail!(
                    "app-server broker lifecycle validation failed before passthrough: {failure}"
                );
            }
            if let Some(failure) = request_response_failure {
                anyhow::bail!(
                    "app-server broker request/response validation failed before passthrough: {failure}"
                );
            }
            if let Some(failure) = lifecycle_payload_failure {
                anyhow::bail!(
                    "app-server broker lifecycle payload validation failed before passthrough: {failure}"
                );
            }
            anyhow::bail!(
                "app-server broker validation failed before passthrough: line={line_index}"
            );
        }
        passthrough_writer.write_all(raw_line.as_bytes())?;
    }
    let summary = app_server_broker_preview_report_json_from_previews(previews);
    app_server_broker_log_preview_summary(&log_path, &summary);
    app_server_broker_audit_preview_summary(mode, &summary);
    serde_json::to_writer(&mut diagnostics_writer, &summary)?;
    diagnostics_writer.write_all(b"\n")?;
    if let Some(failure) = request_response.finish(line_index) {
        anyhow::bail!("app-server broker request/response validation failed at EOF: {failure}");
    }
    Ok(())
}

fn app_server_broker_validation_failure_counts(summary: &Value) -> (u64, u64) {
    let report = &summary["report"];
    let parse_error_count = report["error_count"].as_u64().unwrap_or_default();
    let invalid_frame_count = report["frame_kind_counts"]["invalid"]
        .as_u64()
        .unwrap_or_default();
    (parse_error_count, invalid_frame_count)
}

struct AppServerBrokerLifecyclePayloadValidation;

impl AppServerBrokerLifecyclePayloadValidation {
    fn observe_preview_and_frame(
        &mut self,
        preview: &Value,
        frame: Option<&Value>,
    ) -> Option<String> {
        if !preview["preview"]["parse_ok"].as_bool().unwrap_or_default() {
            return None;
        }
        match preview["preview"]["summary"]["lifecycle_stage"].as_str()? {
            "thread_started_notification"
            | "thread_resume_request"
            | "thread_fork_request"
            | "turn_start_request" => {
                if app_server_broker_preview_thread_id(preview).is_none() {
                    return Some(app_server_broker_lifecycle_payload_failure(
                        preview,
                        "lifecycle_missing_thread_id",
                    ));
                }
            }
            _ => {}
        }
        if preview["preview"]["summary"]["lifecycle_stage"].as_str()?
            == "thread_started_notification"
            && frame
                .and_then(|frame| {
                    app_server_broker_frame_string(frame, &["params", "thread", "id"])
                })
                .is_none()
        {
            return Some(app_server_broker_lifecycle_payload_failure(
                preview,
                "lifecycle_missing_thread_object_id",
            ));
        }
        if preview["preview"]["summary"]["lifecycle_stage"].as_str()?
            == "thread_started_notification"
        {
            if let Some(reason) = frame
                .map(|frame| {
                    app_server_broker_thread_status_failure_reason(
                        frame,
                        &["params", "thread", "status"],
                    )
                })
                .unwrap_or(Some("lifecycle_missing_thread_status"))
            {
                return Some(app_server_broker_lifecycle_payload_failure(preview, reason));
            }
            if !frame.is_some_and(|frame| {
                app_server_broker_thread_object_has_context(frame, &["params", "thread"])
            }) {
                return Some(app_server_broker_lifecycle_payload_failure(
                    preview,
                    "lifecycle_missing_thread_context",
                ));
            }
        }
        if preview["preview"]["summary"]["lifecycle_stage"].as_str()? == "turn_start_request"
            && !frame
                .and_then(|frame| frame.get("params"))
                .and_then(|params| params.get("input"))
                .is_some_and(Value::is_array)
        {
            return Some(app_server_broker_lifecycle_payload_failure(
                preview,
                "lifecycle_missing_turn_input",
            ));
        }
        if matches!(
            preview["preview"]["summary"]["lifecycle_stage"].as_str(),
            Some("turn_started_notification" | "turn_completed_notification")
        ) {
            let turn_status = frame
                .and_then(|frame| frame.get("params"))
                .and_then(|params| params.get("turn"))
                .and_then(|turn| turn.get("status"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|status| !status.is_empty());
            match turn_status {
                Some(status) if app_server_broker_is_valid_turn_status(status) => {}
                Some(_) => {
                    return Some(app_server_broker_lifecycle_payload_failure(
                        preview,
                        "lifecycle_invalid_turn_status",
                    ));
                }
                None => {
                    return Some(app_server_broker_lifecycle_payload_failure(
                        preview,
                        "lifecycle_missing_turn_status",
                    ));
                }
            }
            if !frame
                .and_then(|frame| frame.get("params"))
                .and_then(|params| params.get("turn"))
                .and_then(|turn| turn.get("items"))
                .is_some_and(Value::is_array)
            {
                return Some(app_server_broker_lifecycle_payload_failure(
                    preview,
                    "lifecycle_missing_turn_items",
                ));
            }
        }
        None
    }
}

#[derive(Default)]
struct AppServerBrokerRequestResponseValidation {
    pending_requests: HashMap<String, Option<String>>,
}

impl AppServerBrokerRequestResponseValidation {
    fn annotate_response_schema(&self, preview: &mut Value, frame: Option<&Value>) {
        if preview["preview"]["summary"]["frame_kind"].as_str() != Some("response") {
            return;
        }
        if !frame
            .is_some_and(|frame| frame.get("result").is_some() && !frame.get("error").is_some())
        {
            return;
        }
        let Some(id) = app_server_broker_preview_id_key(preview) else {
            return;
        };
        let Some(Some(lifecycle_stage)) = self.pending_requests.get(&id) else {
            return;
        };
        if let Some(schema_file) = app_server_broker_lifecycle_response_schema_file(lifecycle_stage)
        {
            preview["preview"]["summary"]["lifecycle_schema_file"] =
                Value::String(schema_file.to_string());
        }
    }

    fn observe_for_schema_tracking(&mut self, preview: &Value) {
        if !preview["preview"]["parse_ok"].as_bool().unwrap_or_default() {
            return;
        }
        match preview["preview"]["summary"]["frame_kind"].as_str() {
            Some("request") => {
                if let Some(id) = app_server_broker_preview_id_key(preview) {
                    let lifecycle_stage = preview["preview"]["summary"]["lifecycle_stage"]
                        .as_str()
                        .map(str::to_string);
                    self.pending_requests.insert(id, lifecycle_stage);
                }
            }
            Some("response") => {
                if let Some(id) = app_server_broker_preview_id_key(preview) {
                    self.pending_requests.remove(&id);
                }
            }
            _ => {}
        }
    }

    fn observe_preview_and_frame(
        &mut self,
        preview: &Value,
        frame: Option<&Value>,
    ) -> Option<String> {
        if !preview["preview"]["parse_ok"].as_bool().unwrap_or_default() {
            return None;
        }
        match preview["preview"]["summary"]["frame_kind"].as_str()? {
            "request" => self.observe_request(preview),
            "response" => self.observe_response(preview, frame),
            _ => None,
        }
    }

    fn observe_request(&mut self, preview: &Value) -> Option<String> {
        let Some(id) = app_server_broker_preview_id_key(preview) else {
            return Some(app_server_broker_request_response_failure(
                preview,
                "request_missing_id",
                None,
            ));
        };
        let lifecycle_stage = preview["preview"]["summary"]["lifecycle_stage"]
            .as_str()
            .map(str::to_string);
        if self
            .pending_requests
            .insert(id.clone(), lifecycle_stage)
            .is_some()
        {
            return Some(app_server_broker_request_response_failure(
                preview,
                "duplicate_pending_request_id",
                Some(&id),
            ));
        }
        None
    }

    fn observe_response(&mut self, preview: &Value, frame: Option<&Value>) -> Option<String> {
        let Some(id) = app_server_broker_preview_id_key(preview) else {
            return Some(app_server_broker_request_response_failure(
                preview,
                "response_missing_id",
                None,
            ));
        };
        let Some(lifecycle_stage) = self.pending_requests.remove(&id) else {
            return Some(app_server_broker_request_response_failure(
                preview,
                "response_without_request",
                Some(&id),
            ));
        };
        self.validate_lifecycle_response(preview, frame, lifecycle_stage.as_deref())
    }

    fn validate_lifecycle_response(
        &self,
        preview: &Value,
        frame: Option<&Value>,
        lifecycle_stage: Option<&str>,
    ) -> Option<String> {
        let frame = frame?;
        if frame.get("error").is_some() {
            return None;
        }
        match lifecycle_stage {
            Some("thread_start_request" | "thread_resume_request" | "thread_fork_request") => {
                if app_server_broker_frame_string(frame, &["result", "thread", "id"]).is_none() {
                    return Some(app_server_broker_request_response_failure(
                        preview,
                        "lifecycle_response_missing_thread_id",
                        None,
                    ));
                }
                if let Some(reason) = app_server_broker_thread_status_failure_reason(
                    frame,
                    &["result", "thread", "status"],
                ) {
                    return Some(app_server_broker_request_response_failure(
                        preview,
                        match reason {
                            "lifecycle_invalid_thread_status" => {
                                "lifecycle_response_invalid_thread_status"
                            }
                            _ => "lifecycle_response_missing_thread_status",
                        },
                        None,
                    ));
                }
                if !app_server_broker_thread_response_has_context(frame) {
                    return Some(app_server_broker_request_response_failure(
                        preview,
                        "lifecycle_response_missing_thread_context",
                        None,
                    ));
                }
                if !app_server_broker_thread_response_has_valid_context(frame) {
                    return Some(app_server_broker_request_response_failure(
                        preview,
                        "lifecycle_response_invalid_thread_context",
                        None,
                    ));
                }
                if !app_server_broker_thread_object_has_context(frame, &["result", "thread"]) {
                    return Some(app_server_broker_request_response_failure(
                        preview,
                        "lifecycle_response_missing_thread_object_context",
                        None,
                    ));
                }
            }
            Some("turn_start_request") => {
                if app_server_broker_frame_string(frame, &["result", "turn", "id"]).is_none() {
                    return Some(app_server_broker_request_response_failure(
                        preview,
                        "lifecycle_response_missing_turn_id",
                        None,
                    ));
                }
                match app_server_broker_frame_string(frame, &["result", "turn", "status"])
                    .as_deref()
                {
                    Some(status) if app_server_broker_is_valid_turn_status(status) => {}
                    Some(_) => {
                        return Some(app_server_broker_request_response_failure(
                            preview,
                            "lifecycle_response_invalid_turn_status",
                            None,
                        ));
                    }
                    None => {
                        return Some(app_server_broker_request_response_failure(
                            preview,
                            "lifecycle_response_missing_turn_status",
                            None,
                        ));
                    }
                }
                if !app_server_broker_frame_value(frame, &["result", "turn", "items"])
                    .is_some_and(Value::is_array)
                {
                    return Some(app_server_broker_request_response_failure(
                        preview,
                        "lifecycle_response_missing_turn_items",
                        None,
                    ));
                }
            }
            _ => {}
        };
        None
    }

    fn finish(&self, line_index: usize) -> Option<String> {
        let id = self.pending_requests.keys().min()?;
        Some(format!(
            "line={line_index} reason=pending_request_without_response id={id}"
        ))
    }
}

#[derive(Default)]
struct AppServerBrokerLifecycleValidation {
    active_turn_by_thread: HashMap<String, String>,
    started_turns: HashSet<String>,
    completed_turns: HashSet<String>,
}

impl AppServerBrokerLifecycleValidation {
    fn observe_preview(&mut self, preview: &Value) -> Option<String> {
        if !preview["preview"]["parse_ok"].as_bool().unwrap_or_default() {
            return None;
        }
        match preview["preview"]["summary"]["lifecycle_stage"].as_str()? {
            "turn_started_notification" => self.observe_turn_started(preview),
            "turn_completed_notification" => self.observe_turn_completed(preview),
            "turn_interrupt_request" => self.observe_turn_interrupt(preview),
            _ => None,
        }
    }

    fn observe_turn_started(&mut self, preview: &Value) -> Option<String> {
        let Some(turn_id) = app_server_broker_preview_turn_id(preview) else {
            return Some(app_server_broker_lifecycle_failure(
                preview,
                "turn_started_missing_turn_id",
                None,
            ));
        };
        let Some(thread_id) = app_server_broker_preview_thread_id(preview) else {
            return Some(app_server_broker_lifecycle_failure(
                preview,
                "turn_started_missing_thread_id",
                Some(&turn_id),
            ));
        };
        if self.completed_turns.contains(&turn_id) {
            return Some(app_server_broker_lifecycle_failure(
                preview,
                "turn_started_after_completed",
                Some(&turn_id),
            ));
        }
        if let Some(active_turn_id) = self.active_turn_by_thread.get(&thread_id)
            && active_turn_id != &turn_id
        {
            return Some(format!(
                "line={} reason=thread_active_turn_conflict thread_id={} active_turn_id={} turn_id={}",
                preview["line"].as_u64().unwrap_or_default(),
                thread_id,
                active_turn_id,
                turn_id
            ));
        }
        if !self.started_turns.insert(turn_id.clone()) {
            return Some(app_server_broker_lifecycle_failure(
                preview,
                "duplicate_turn_started",
                Some(&turn_id),
            ));
        }
        self.active_turn_by_thread.insert(thread_id, turn_id);
        None
    }

    fn observe_turn_completed(&mut self, preview: &Value) -> Option<String> {
        let Some(turn_id) = app_server_broker_preview_turn_id(preview) else {
            return Some(app_server_broker_lifecycle_failure(
                preview,
                "turn_completed_missing_turn_id",
                None,
            ));
        };
        let Some(thread_id) = app_server_broker_preview_thread_id(preview) else {
            return Some(app_server_broker_lifecycle_failure(
                preview,
                "turn_completed_missing_thread_id",
                Some(&turn_id),
            ));
        };
        if !self.started_turns.contains(&turn_id) {
            return Some(app_server_broker_lifecycle_failure(
                preview,
                "turn_completed_without_turn_started",
                Some(&turn_id),
            ));
        }
        if self
            .active_turn_by_thread
            .get(&thread_id)
            .is_some_and(|active_turn_id| active_turn_id != &turn_id)
        {
            return Some(format!(
                "line={} reason=turn_completed_not_active thread_id={} turn_id={}",
                preview["line"].as_u64().unwrap_or_default(),
                thread_id,
                turn_id
            ));
        }
        if !self.completed_turns.insert(turn_id.clone()) {
            return Some(app_server_broker_lifecycle_failure(
                preview,
                "duplicate_turn_completed",
                Some(&turn_id),
            ));
        }
        self.active_turn_by_thread.remove(&thread_id);
        None
    }

    fn observe_turn_interrupt(&mut self, preview: &Value) -> Option<String> {
        let Some(turn_id) = app_server_broker_preview_turn_id(preview) else {
            return Some(app_server_broker_lifecycle_failure(
                preview,
                "turn_interrupt_missing_turn_id",
                None,
            ));
        };
        let Some(thread_id) = app_server_broker_preview_thread_id(preview) else {
            return Some(app_server_broker_lifecycle_failure(
                preview,
                "turn_interrupt_missing_thread_id",
                Some(&turn_id),
            ));
        };
        if let Some(active_turn_id) = self.active_turn_by_thread.get(&thread_id) {
            if active_turn_id != &turn_id {
                return Some(format!(
                    "line={} reason=turn_interrupt_active_turn_conflict thread_id={} active_turn_id={} turn_id={}",
                    preview["line"].as_u64().unwrap_or_default(),
                    thread_id,
                    active_turn_id,
                    turn_id
                ));
            }
            self.active_turn_by_thread.remove(&thread_id);
        }
        None
    }
}

fn app_server_broker_preview_thread_id(preview: &Value) -> Option<String> {
    preview["preview"]["summary"]["metadata"]["thread_id"]
        .as_str()
        .map(str::trim)
        .filter(|thread_id| !thread_id.is_empty())
        .map(str::to_string)
}

fn app_server_broker_preview_id_key(preview: &Value) -> Option<String> {
    let id = &preview["preview"]["summary"]["id"];
    if id.is_null() {
        None
    } else {
        Some(id.to_string())
    }
}

fn app_server_broker_frame_string(frame: &Value, path: &[&str]) -> Option<String> {
    let mut current = frame;
    for segment in path {
        current = current.get(*segment)?;
    }
    current
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn app_server_broker_frame_value<'a>(frame: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = frame;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn app_server_broker_thread_response_has_context(frame: &Value) -> bool {
    ["cwd", "model", "modelProvider"]
        .iter()
        .all(|field| app_server_broker_frame_string(frame, &["result", field]).is_some())
        && ["approvalPolicy", "approvalsReviewer", "sandbox"]
            .iter()
            .all(|field| {
                app_server_broker_frame_value(frame, &["result", field])
                    .is_some_and(|value| !value.is_null())
            })
}

fn app_server_broker_thread_response_has_valid_context(frame: &Value) -> bool {
    app_server_broker_frame_value(frame, &["result", "approvalPolicy"])
        .is_some_and(app_server_broker_is_valid_approval_policy)
        && app_server_broker_frame_string(frame, &["result", "approvalsReviewer"])
            .is_some_and(|value| app_server_broker_is_valid_approvals_reviewer(&value))
        && app_server_broker_frame_string(frame, &["result", "sandbox", "type"])
            .is_some_and(|value| app_server_broker_is_valid_sandbox_type(&value))
}

fn app_server_broker_thread_object_has_context(frame: &Value, path: &[&str]) -> bool {
    let Some(thread) = app_server_broker_frame_value(frame, path) else {
        return false;
    };
    ["cliVersion", "cwd", "modelProvider", "sessionId"]
        .iter()
        .all(|field| {
            thread
                .get(*field)
                .and_then(Value::as_str)
                .map(str::trim)
                .is_some_and(|value| !value.is_empty())
        })
        && ["createdAt", "updatedAt"]
            .iter()
            .all(|field| thread.get(*field).is_some_and(Value::is_number))
        && thread
            .get("source")
            .is_some_and(app_server_broker_is_valid_session_source)
        && thread.get("preview").is_some_and(Value::is_string)
        && thread.get("ephemeral").is_some_and(Value::is_boolean)
        && thread.get("turns").is_some_and(Value::is_array)
}

fn app_server_broker_is_valid_session_source(value: &Value) -> bool {
    match value {
        Value::String(value) => matches!(
            value.trim(),
            "cli" | "vscode" | "exec" | "appServer" | "unknown"
        ),
        Value::Object(source) => {
            source.get("custom").is_some_and(Value::is_string)
                || source.get("subAgent").is_some_and(|value| !value.is_null())
        }
        _ => false,
    }
}

fn app_server_broker_is_valid_approval_policy(value: &Value) -> bool {
    match value {
        Value::String(value) => matches!(value.trim(), "untrusted" | "on-request" | "never"),
        Value::Object(_) => true,
        _ => false,
    }
}

fn app_server_broker_is_valid_approvals_reviewer(value: &str) -> bool {
    matches!(value, "user" | "auto_review" | "guardian_subagent")
}

fn app_server_broker_is_valid_sandbox_type(value: &str) -> bool {
    matches!(
        value,
        "dangerFullAccess" | "readOnly" | "externalSandbox" | "workspaceWrite"
    )
}

fn app_server_broker_is_valid_turn_status(status: &str) -> bool {
    matches!(
        status,
        "completed" | "interrupted" | "failed" | "inProgress"
    )
}

fn app_server_broker_is_valid_thread_status(status: &str) -> bool {
    matches!(status, "notLoaded" | "idle" | "systemError" | "active")
}

fn app_server_broker_thread_status_failure_reason(
    frame: &Value,
    path: &[&str],
) -> Option<&'static str> {
    let Some(status) = app_server_broker_frame_value(frame, path) else {
        return Some("lifecycle_missing_thread_status");
    };
    let Some(status_type) = status
        .get("type")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Some("lifecycle_missing_thread_status");
    };
    if !app_server_broker_is_valid_thread_status(status_type) {
        return Some("lifecycle_invalid_thread_status");
    }
    if status_type == "active" {
        let Some(active_flags) = status.get("activeFlags").and_then(Value::as_array) else {
            return Some("lifecycle_invalid_thread_status");
        };
        if !active_flags.iter().all(|flag| {
            flag.as_str()
                .is_some_and(app_server_broker_is_valid_thread_active_flag)
        }) {
            return Some("lifecycle_invalid_thread_status");
        }
    }
    None
}

fn app_server_broker_is_valid_thread_active_flag(flag: &str) -> bool {
    matches!(flag, "waitingOnApproval" | "waitingOnUserInput")
}

fn app_server_broker_request_response_failure(
    preview: &Value,
    reason: &'static str,
    id: Option<&str>,
) -> String {
    let line = preview["line"].as_u64().unwrap_or_default();
    match id {
        Some(id) => format!("line={line} reason={reason} id={id}"),
        None => format!("line={line} reason={reason}"),
    }
}

fn app_server_broker_lifecycle_payload_failure(preview: &Value, reason: &'static str) -> String {
    let line = preview["line"].as_u64().unwrap_or_default();
    let stage = preview["preview"]["summary"]["lifecycle_stage"]
        .as_str()
        .unwrap_or("unknown");
    format!("line={line} reason={reason} lifecycle_stage={stage}")
}

fn app_server_broker_preview_turn_id(preview: &Value) -> Option<String> {
    preview["preview"]["summary"]["metadata"]["turn_id"]
        .as_str()
        .map(str::trim)
        .filter(|turn_id| !turn_id.is_empty())
        .map(str::to_string)
}

fn app_server_broker_lifecycle_failure(
    preview: &Value,
    reason: &'static str,
    turn_id: Option<&str>,
) -> String {
    let line = preview["line"].as_u64().unwrap_or_default();
    match turn_id {
        Some(turn_id) => format!("line={line} reason={reason} turn_id={turn_id}"),
        None => format!("line={line} reason={reason}"),
    }
}

fn app_server_broker_write_stdio_preview_stream_with_mode<R: BufRead, W: Write, D: Write>(
    mut reader: R,
    mut passthrough_writer: W,
    mut diagnostics_writer: D,
    mode: &'static str,
) -> anyhow::Result<Value> {
    let log_path = initialize_runtime_proxy_log_path();
    let mut previews = Vec::new();
    let mut request_response = AppServerBrokerRequestResponseValidation::default();
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
        passthrough_writer.write_all(raw_line.as_bytes())?;
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let mut preview = serde_json::json!({
            "object": "app_server_broker.preview_event",
            "line": line_index,
            "preview": app_server_broker_preview_line(line),
        });
        let frame = serde_json::from_str::<Value>(line).ok();
        request_response.annotate_response_schema(&mut preview, frame.as_ref());
        app_server_broker_log_preview_event(&log_path, line_index, &preview);
        serde_json::to_writer(&mut diagnostics_writer, &preview)?;
        diagnostics_writer.write_all(b"\n")?;
        request_response.observe_for_schema_tracking(&preview);
        previews.push(preview);
    }
    let summary = app_server_broker_preview_report_json_from_previews(previews);
    app_server_broker_log_preview_summary(&log_path, &summary);
    app_server_broker_audit_preview_summary(mode, &summary);
    serde_json::to_writer(&mut diagnostics_writer, &summary)?;
    diagnostics_writer.write_all(b"\n")?;
    Ok(summary)
}

pub(crate) fn app_server_broker_status_line() -> String {
    let contract = app_server_broker_contract_json();
    let status = contract
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let transport = contract
        .get("transport")
        .and_then(Value::as_array)
        .map(|transport| {
            transport
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|transport| !transport.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let default_mode = contract
        .get("default_mode")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    format!(
        "app-server broker: status={status}; transport={transport}; mode={default_mode}; passthrough-aware stdio preview is available; default Codex app-server passthrough remains active"
    )
}

pub(crate) fn app_server_broker_render_output(json: bool) -> anyhow::Result<String> {
    if json {
        Ok(serde_json::to_string_pretty(
            &app_server_broker_contract_json(),
        )?)
    } else {
        Ok(app_server_broker_status_line())
    }
}

pub(crate) fn app_server_broker_render_stdio_preview(input: &str) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(
        &app_server_broker_preview_report_json(input),
    )?)
}

pub(crate) fn app_server_broker_contract_json() -> serde_json::Value {
    serde_json::json!({
        "object": "app_server_broker.contract",
        "enabled_by_default": false,
        "status": "diagnostic-envelope-parsing",
        "transport": ["stdio-preview", "stdio-passthrough-preview", "stdio-validate", "stdio-validate-passthrough", "stdio-live"],
        "jsonrpc": "2.0",
        "wire_omits_jsonrpc_header": true,
        "default_mode": "direct-passthrough",
        "schema_validation": {
            "protocol_surface_fixture": true,
            "fixture_drift_tests": true,
            "helper_consistency_tests": true,
            "stream_helper_consistency_tests": true,
            "cross_surface_consistency_tests": true,
            "report_aggregation_consistency_tests": true,
            "metadata_surface_fixture": true,
            "metadata_drift_tests": true,
            "output_surface_fixture": true,
            "output_drift_tests": true,
            "runtime_log_surface_fixture": true,
            "runtime_log_drift_tests": true,
            "upstream_codex_schema_imported": true,
            "lifecycle_schema_hints": true,
            "lifecycle_response_schema_hints": true
        },
        "cli": {
            "json_contract": true,
            "experimental_stdio_preview": true,
            "experimental_stdio_live": true,
            "experimental_stdio_passthrough_preview": true,
            "experimental_stdio_validate": true,
            "experimental_stdio_validate_passthrough": true
        },
        "diagnostics": {
            "jsonrpc_envelope_validation": true,
            "max_preview_line_bytes": APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES,
            "frame_kinds": ["request", "notification", "response", "invalid"],
            "method_kinds": ["lifecycle", "other", "absent"],
            "invalid_reasons": [
                "non_jsonrpc_version",
                "batch_frame_unsupported",
                "non_object_frame",
                "non_scalar_id",
                "non_container_params",
                "non_object_error",
                "non_integer_error_code",
                "non_string_error_message",
                "non_string_method",
                "invalid_method_name",
                "result_with_error",
                "missing_response_id",
                "method_with_result_or_error",
                "missing_method_and_response_payload"
            ],
            "metadata_extraction": ["session_id", "thread_id", "turn_id", "item_id"],
            "replay_summary": true,
            "request_summary_json": true,
            "response_summary_json": true,
            "policy_hint": true,
            "commit_boundary_hint": true,
            "turn_committed_hint": true,
            "affinity_required_hint": true,
            "rotation_window_hint": true,
            "rotation_allowed_hint": true,
            "routing_hint": true,
            "provider_switch_policy_hint": true,
            "preserved_owner_kind_hint": true,
            "preserves_owner_hint": true,
            "policy_flag_counts": true,
            "audit_preview_summary": true,
            "stdio_preview_report": true,
            "stdio_preview_report_envelope": true,
            "stdio_preview_pretty_json": true,
            "stdio_preview_jsonl": true,
            "stdio_preview_session_summary": true,
            "stdio_passthrough_preview": true,
            "stdio_passthrough_preserves_input": true,
            "stdio_passthrough_empty_summary": true,
            "stdio_passthrough_write_failures_surface": true,
            "stdio_diagnostics_write_failures_surface": true,
            "stdio_validate_fail_closed": true,
            "stdio_validate_passthrough_fail_closed": true,
            "stdio_validate_passthrough_preserves_valid_input": true,
            "request_response_id_validation": true,
            "request_response_validation_reasons": [
                "request_missing_id",
                "response_missing_id",
                "response_without_request",
                "duplicate_pending_request_id",
                "pending_request_without_response",
                "lifecycle_response_missing_thread_id",
                "lifecycle_response_missing_thread_status",
                "lifecycle_response_invalid_thread_status",
                "lifecycle_response_missing_thread_context",
                "lifecycle_response_invalid_thread_context",
                "lifecycle_response_missing_thread_object_context",
                "lifecycle_response_missing_turn_id",
                "lifecycle_response_missing_turn_items",
                "lifecycle_response_missing_turn_status",
                "lifecycle_response_invalid_turn_status"
            ],
            "lifecycle_payload_validation": true,
            "lifecycle_payload_validation_reasons": [
                "lifecycle_missing_thread_id",
                "lifecycle_missing_thread_object_id",
                "lifecycle_missing_thread_context",
                "lifecycle_missing_thread_status",
                "lifecycle_invalid_thread_status",
                "lifecycle_missing_turn_input",
                "lifecycle_missing_turn_items",
                "lifecycle_invalid_turn_status",
                "lifecycle_missing_turn_status"
            ],
            "lifecycle_consistency_validation": true,
            "lifecycle_validation_reasons": [
                "turn_started_missing_turn_id",
                "turn_started_missing_thread_id",
                "turn_completed_missing_turn_id",
                "turn_completed_missing_thread_id",
                "turn_interrupt_missing_turn_id",
                "turn_interrupt_missing_thread_id",
                "turn_completed_without_turn_started",
                "turn_started_after_completed",
                "thread_active_turn_conflict",
                "turn_completed_not_active",
                "turn_interrupt_active_turn_conflict",
                "duplicate_turn_started",
                "duplicate_turn_completed"
            ],
            "invalid_frame_reasoning": "shape-based",
            "routing_changes": false
        },
        "lifecycle_methods": app_server_broker_lifecycle_methods(),
        "accepted_lifecycle_aliases": ["notifications/initialized", "turn/cancel"],
        "affinity": {
            "thread_session_owner_required": true,
            "continuation_affinity_wins": true,
            "rotate_only_before_turn_commit": true,
            "decision_kinds": app_server_broker_continuation_decision_kinds(),
            "policy_modes": app_server_broker_policy_modes(),
            "routing_hints": app_server_broker_routing_hints(),
            "commit_boundaries": app_server_broker_commit_boundaries(),
            "rotation_windows": app_server_broker_rotation_windows()
        },
        "audit": {
            "preview_summary": {
                "component": "app_server_broker",
                "action": "preview_session",
                "outcome": "observed",
                "modes": [
                    "stdio-preview",
                    "stdio-passthrough-preview",
                    "stdio-validate",
                    "stdio-validate-passthrough",
                    "stdio-live"
                ],
                "counts_only": true,
                "required_fields": app_server_broker_audit_preview_summary_required_fields()
            }
        },
        "errors": {
            "quota": "json-rpc-error-planned",
            "rate_limit": "json-rpc-error-planned",
            "overload": "json-rpc-error-planned"
        }
    })
}

fn app_server_broker_preview_report_json_from_previews(previews: Vec<Value>) -> Value {
    serde_json::json!({
        "object": "app_server_broker.preview_report",
        "report": app_server_broker_preview_report_from_previews(previews),
    })
}
