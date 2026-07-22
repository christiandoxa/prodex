use super::super::logging::{
    app_server_broker_audit_preview_summary, app_server_broker_log_preview_event,
    app_server_broker_log_preview_summary,
};
use super::super::parse::APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES;
use super::super::validation::PreviewSession;
use crate::initialize_runtime_proxy_log_path;
use std::io::{BufRead, Read, Write};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(crate) struct AppServerBrokerLiveValidator {
    state: Arc<Mutex<AppServerBrokerLiveValidationState>>,
}

struct AppServerBrokerLiveValidationState {
    session: PreviewSession,
    line_index: usize,
    finished: bool,
    log_path: std::path::PathBuf,
}

impl AppServerBrokerLiveValidator {
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(AppServerBrokerLiveValidationState {
                session: PreviewSession::default(),
                line_index: 0,
                finished: false,
                log_path: initialize_runtime_proxy_log_path(),
            })),
        }
    }

    fn validate_line<D: Write>(
        &self,
        direction: &'static str,
        line: &str,
        diagnostics: &mut D,
    ) -> anyhow::Result<()> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(());
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("app-server broker live validation lock poisoned"))?;
        if state.finished {
            anyhow::bail!("app-server broker live validation already stopped");
        }
        state.line_index = state.line_index.saturating_add(1);
        let line_index = state.line_index;
        let mut observation = state.session.validate_line(line_index, line);
        observation.preview["direction"] = direction.into();
        app_server_broker_log_preview_event(&state.log_path, line_index, &observation.preview);
        serde_json::to_writer(&mut *diagnostics, &observation.preview)?;
        diagnostics.write_all(b"\n")?;
        diagnostics.flush()?;

        let parse_failed = !observation.preview["preview"]["parse_ok"]
            .as_bool()
            .unwrap_or_default();
        let invalid_frame = observation.preview["preview"]["summary"]["frame_kind"]
            .as_str()
            .is_some_and(|kind| kind == "invalid");
        let failure = observation
            .lifecycle_failure
            .or(observation.request_response_failure)
            .or(observation.lifecycle_payload_failure)
            .map(|failure| failure.to_string());
        if parse_failed || invalid_frame || failure.is_some() {
            let reason = failure.unwrap_or_else(|| "invalid protocol frame".to_string());
            finish_locked(&mut state, diagnostics)?;
            anyhow::bail!(
                "app-server broker live validation failed direction={direction}: {reason}"
            );
        }
        Ok(())
    }

    pub(crate) fn finish<D: Write>(&self, diagnostics: &mut D) -> anyhow::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("app-server broker live validation lock poisoned"))?;
        if state.finished {
            return Ok(());
        }
        if let Some(failure) = state.session.finish(state.line_index) {
            finish_locked(&mut state, diagnostics)?;
            anyhow::bail!("app-server broker live validation failed at EOF: {failure}");
        }
        finish_locked(&mut state, diagnostics)
    }
}

fn finish_locked<D: Write>(
    state: &mut AppServerBrokerLiveValidationState,
    diagnostics: &mut D,
) -> anyhow::Result<()> {
    if state.finished {
        return Ok(());
    }
    state.finished = true;
    let summary = std::mem::take(&mut state.session).into_report_json();
    app_server_broker_log_preview_summary(&state.log_path, &summary);
    app_server_broker_audit_preview_summary("stdio-live", &summary)?;
    serde_json::to_writer(&mut *diagnostics, &summary)?;
    diagnostics.write_all(b"\n")?;
    diagnostics.flush()?;
    Ok(())
}

pub(crate) fn app_server_broker_pump_live_stream<R, W, D>(
    mut reader: R,
    mut writer: W,
    validator: AppServerBrokerLiveValidator,
    diagnostics: Arc<Mutex<D>>,
    direction: &'static str,
) -> anyhow::Result<()>
where
    R: BufRead,
    W: Write,
    D: Write,
{
    let mut raw_line = String::new();
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
                "app-server broker live line exceeds {} bytes before newline",
                APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES
            );
        }
        {
            let mut diagnostics = diagnostics
                .lock()
                .map_err(|_| anyhow::anyhow!("app-server broker live diagnostics lock poisoned"))?;
            validator.validate_line(direction, &raw_line, &mut *diagnostics)?;
        }
        writer.write_all(raw_line.as_bytes())?;
        writer.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AppServerBrokerLiveValidator, app_server_broker_pump_live_stream};
    use std::sync::{Arc, Mutex};

    #[test]
    fn duplex_live_validation_tracks_requests_across_directions() {
        let validator = AppServerBrokerLiveValidator::new();
        let diagnostics = Arc::new(Mutex::new(Vec::new()));
        let request = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"custom/ping\",\"params\":{}}\n";
        let response = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"ok\":true}}\n";
        let mut forwarded_request = Vec::new();
        let mut forwarded_response = Vec::new();

        app_server_broker_pump_live_stream(
            std::io::Cursor::new(request),
            &mut forwarded_request,
            validator.clone(),
            Arc::clone(&diagnostics),
            "client_to_server",
        )
        .unwrap();
        app_server_broker_pump_live_stream(
            std::io::Cursor::new(response),
            &mut forwarded_response,
            validator.clone(),
            Arc::clone(&diagnostics),
            "server_to_client",
        )
        .unwrap();
        validator.finish(&mut *diagnostics.lock().unwrap()).unwrap();

        assert_eq!(forwarded_request, request);
        assert_eq!(forwarded_response, response);
        let diagnostics =
            String::from_utf8(Arc::try_unwrap(diagnostics).unwrap().into_inner().unwrap()).unwrap();
        assert!(diagnostics.contains("client_to_server"));
        assert!(diagnostics.contains("server_to_client"));
    }

    #[test]
    fn duplex_live_validation_rejects_before_forwarding() {
        let validator = AppServerBrokerLiveValidator::new();
        let diagnostics = Arc::new(Mutex::new(Vec::new()));
        let mut forwarded = Vec::new();
        let error = app_server_broker_pump_live_stream(
            std::io::Cursor::new(b"{not-json}\n"),
            &mut forwarded,
            validator,
            diagnostics,
            "server_to_client",
        )
        .unwrap_err();

        assert!(error.to_string().contains("live validation failed"));
        assert!(forwarded.is_empty());
    }
}
