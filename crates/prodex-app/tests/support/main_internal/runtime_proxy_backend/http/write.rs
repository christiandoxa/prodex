use super::*;

pub(crate) struct RuntimeProxyBackendHttpResponse {
    pub(super) status_line: &'static str,
    pub(super) content_type: &'static str,
    pub(super) body: String,
    pub(super) response_turn_state: Option<String>,
    pub(super) initial_body_stall: Option<Duration>,
    pub(super) chunk_delay: Option<Duration>,
}

impl RuntimeProxyBackendHttpResponse {
    pub(crate) fn new(
        status_line: &'static str,
        content_type: &'static str,
        body: String,
        response_turn_state: Option<String>,
        initial_body_stall: Option<Duration>,
        chunk_delay: Option<Duration>,
    ) -> Self {
        Self {
            status_line,
            content_type,
            body,
            response_turn_state,
            initial_body_stall,
            chunk_delay,
        }
    }
}

pub(super) fn write_runtime_proxy_backend_http_response(
    mut stream: TcpStream,
    response: RuntimeProxyBackendHttpResponse,
    account_id: &str,
    mode: RuntimeProxyBackendMode,
) {
    let RuntimeProxyBackendHttpResponse {
        status_line,
        content_type,
        body,
        response_turn_state,
        initial_body_stall,
        chunk_delay,
    } = response;

    let mut headers = format!(
        "{status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len(),
    );
    if let Some(turn_state) = response_turn_state.as_deref() {
        headers.push_str(&format!("x-codex-turn-state: {turn_state}\r\n"));
    }
    headers.push_str("\r\n");
    let _ = stream.write_all(headers.as_bytes());
    let _ = stream.flush();
    if let Some(delay) = initial_body_stall {
        thread::sleep(delay);
    }
    if content_type == "text/event-stream"
        && matches!(mode, RuntimeProxyBackendMode::HttpOnlyResetAfterFirstChunk)
        && account_id == "second-account"
    {
        let reset_chunk = body
            .split("\r\n\r\n")
            .next()
            .map(|chunk| format!("{chunk}\r\n\r\n"))
            .unwrap_or_else(|| body.clone());
        let _ = stream.write_all(reset_chunk.as_bytes());
        let _ = stream.flush();
        return;
    }
    if content_type == "text/event-stream"
        && matches!(
            mode,
            RuntimeProxyBackendMode::HttpOnlyPreviousResponseNotFoundAfterCommit
        )
        && account_id == "second-account"
    {
        for (index, event) in body
            .split("\r\n\r\n")
            .filter(|event| !event.is_empty())
            .enumerate()
        {
            let _ = stream.write_all(format!("{event}\r\n\r\n").as_bytes());
            let _ = stream.flush();
            if index == 1 {
                thread::sleep(Duration::from_millis(50));
            }
        }
        return;
    }
    if content_type == "text/event-stream"
        && let Some(delay) = chunk_delay
    {
        let body_bytes = body.as_bytes();
        let chunk_size = body_bytes.len().max(1).div_ceil(4);
        for (index, chunk) in body_bytes.chunks(chunk_size).enumerate() {
            let _ = stream.write_all(chunk);
            let _ = stream.flush();
            if matches!(
                mode,
                RuntimeProxyBackendMode::HttpOnlyStallAfterSeveralChunks
            ) && account_id == "second-account"
                && index >= 1
            {
                thread::sleep(Duration::from_millis(
                    runtime_proxy_stream_idle_timeout_ms() + 100,
                ));
                return;
            }
            if index + 1 < body_bytes.chunks(chunk_size).len() {
                thread::sleep(delay);
            }
        }
    } else {
        let _ = stream.write_all(body.as_bytes());
        let _ = stream.flush();
    }
}
