use super::*;
use bytes::Bytes;
use prodex_gateway_server::GatewayResponseBodySender;
use redaction::redaction_redact_secret_like_text;
use runtime_proxy_crate::runtime_stream_response_should_flush_each_chunk;

pub(crate) fn write_runtime_streaming_response(
    writer: Box<dyn Write + Send + 'static>,
    response: RuntimeStreamingResponse,
) -> io::Result<()> {
    write_runtime_streaming_response_to_sink(RuntimeHttp1StreamSink { writer }, response)
}

pub(crate) fn write_runtime_gateway_streaming_response(
    sender: GatewayResponseBodySender,
    response: RuntimeStreamingResponse,
) -> io::Result<()> {
    write_runtime_streaming_response_to_sink(
        RuntimeGatewayStreamSink {
            sender: Some(sender),
        },
        response,
    )
}

trait RuntimeStreamingSink {
    fn write_head(&mut self, status: u16, headers: &[(String, String)]) -> io::Result<()>;
    fn write_chunk(&mut self, chunk: &[u8], flush: bool) -> io::Result<()>;
    fn finish(&mut self) -> io::Result<()>;
    fn send_error(&mut self, error: &io::Error);
}

struct RuntimeHttp1StreamSink {
    writer: Box<dyn Write + Send + 'static>,
}

impl RuntimeStreamingSink for RuntimeHttp1StreamSink {
    fn write_head(&mut self, status: u16, headers: &[(String, String)]) -> io::Result<()> {
        let reason = reqwest::StatusCode::from_u16(status)
            .ok()
            .and_then(|status| status.canonical_reason().map(str::to_string))
            .unwrap_or_else(|| "OK".to_string());
        write!(
            self.writer,
            "HTTP/1.1 {status} {reason}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n"
        )?;
        for (name, value) in headers {
            write!(self.writer, "{name}: {value}\r\n")?;
        }
        self.writer.write_all(b"\r\n")?;
        self.writer.flush()
    }

    fn write_chunk(&mut self, chunk: &[u8], flush: bool) -> io::Result<()> {
        write!(self.writer, "{:X}\r\n", chunk.len())?;
        self.writer.write_all(chunk)?;
        self.writer.write_all(b"\r\n")?;
        if flush {
            self.writer.flush()?;
        }
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        self.writer.write_all(b"0\r\n\r\n")?;
        self.writer.flush()
    }

    fn send_error(&mut self, _error: &io::Error) {}
}

struct RuntimeGatewayStreamSink {
    sender: Option<GatewayResponseBodySender>,
}

impl RuntimeStreamingSink for RuntimeGatewayStreamSink {
    fn write_head(&mut self, _status: u16, _headers: &[(String, String)]) -> io::Result<()> {
        Ok(())
    }

    fn write_chunk(&mut self, chunk: &[u8], _flush: bool) -> io::Result<()> {
        self.sender
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "gateway response closed"))?
            .blocking_send(Bytes::copy_from_slice(chunk))
    }

    fn finish(&mut self) -> io::Result<()> {
        self.sender
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "gateway response closed"))?
            .finish()
    }

    fn send_error(&mut self, error: &io::Error) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.blocking_send_error(io::Error::new(error.kind(), error.to_string()));
        }
    }
}

fn write_runtime_streaming_response_to_sink(
    mut sink: impl RuntimeStreamingSink,
    mut response: RuntimeStreamingResponse,
) -> io::Result<()> {
    let flush_each_chunk = runtime_stream_response_should_flush_each_chunk(
        response
            .headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str())),
    );
    let started_at = Instant::now();
    let log_writer_error = |stage: &str,
                            chunk_count: usize,
                            total_bytes: usize,
                            err: &io::Error| {
        runtime_proxy_log_to_path(
            &response.log_path,
            &format!(
                "local_writer_error request={} transport=http profile={} stage={} chunks={} bytes={} elapsed_ms={} error={}",
                response.request_id,
                response.profile_name,
                stage,
                chunk_count,
                total_bytes,
                started_at.elapsed().as_millis(),
                runtime_streaming_error_log_value(err)
            ),
        );
    };
    runtime_proxy_log_to_path(
        &response.log_path,
        &format!(
            "request={} transport=http stream_start profile={} status={}",
            response.request_id, response.profile_name, response.status
        ),
    );
    sink.write_head(response.status, &response.headers)
        .inspect_err(|error| log_writer_error("headers", 0, 0, error))?;

    let mut buffer = [0_u8; 8192];
    let mut total_bytes = 0usize;
    let mut chunk_count = 0usize;
    loop {
        let read = match response.body.read(&mut buffer) {
            Ok(read) => read,
            Err(error) => {
                runtime_stream_read_error(&response, &started_at, chunk_count, total_bytes, &error);
                sink.send_error(&error);
                return Err(error);
            }
        };
        if read == 0 {
            break;
        }
        if chunk_count == 0
            && runtime_take_fault_injection_budget(
                "PRODEX_RUNTIME_FAULT_STREAM_READ_ERROR_ONCE",
                response.shared.runtime_config.fault_stream_read_error_once,
            )
        {
            let error = io::Error::new(
                io::ErrorKind::ConnectionReset,
                "injected runtime stream read failure",
            );
            runtime_stream_read_error(&response, &started_at, chunk_count, total_bytes, &error);
            sink.send_error(&error);
            return Err(error);
        }
        write_runtime_stream_chunk(
            &mut sink,
            &response,
            &started_at,
            &buffer[..read],
            flush_each_chunk,
            &mut chunk_count,
            &mut total_bytes,
        )
        .inspect_err(|error| log_writer_error("chunk", chunk_count, total_bytes, error))?;
    }
    sink.finish()
        .inspect_err(|error| log_writer_error("finish", chunk_count, total_bytes, error))?;
    runtime_proxy_log_to_path(
        &response.log_path,
        &format!(
            "request={} transport=http stream_complete profile={} chunks={} bytes={} elapsed_ms={}",
            response.request_id,
            response.profile_name,
            chunk_count,
            total_bytes,
            started_at.elapsed().as_millis()
        ),
    );
    note_runtime_profile_latency_observation(
        &response.shared,
        &response.profile_name,
        RuntimeRouteKind::Responses,
        "stream_complete",
        started_at.elapsed().as_millis() as u64,
    );
    Ok(())
}

fn runtime_stream_read_error(
    response: &RuntimeStreamingResponse,
    started_at: &Instant,
    chunk_count: usize,
    total_bytes: usize,
    error: &io::Error,
) {
    runtime_proxy_log_to_path(
        &response.log_path,
        &runtime_proxy_structured_log_message(
            "stream_read_error",
            [
                runtime_proxy_log_field("request", response.request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("profile", response.profile_name.as_str()),
                runtime_proxy_log_field("chunks", chunk_count.to_string()),
                runtime_proxy_log_field("bytes", total_bytes.to_string()),
                runtime_proxy_log_field("elapsed_ms", started_at.elapsed().as_millis().to_string()),
                runtime_proxy_log_field("error", runtime_streaming_error_log_value(error)),
            ],
        ),
    );
    let transport_error = anyhow::Error::new(io::Error::new(error.kind(), error.to_string()));
    if is_runtime_proxy_transport_failure(&transport_error) {
        note_runtime_profile_latency_failure(
            &response.shared,
            &response.profile_name,
            RuntimeRouteKind::Responses,
            "stream_read_error",
        );
    }
}

fn write_runtime_stream_chunk(
    sink: &mut impl RuntimeStreamingSink,
    response: &RuntimeStreamingResponse,
    started_at: &Instant,
    chunk: &[u8],
    flush_each_chunk: bool,
    chunk_count: &mut usize,
    total_bytes: &mut usize,
) -> io::Result<()> {
    if chunk.is_empty() {
        return Ok(());
    }
    *chunk_count += 1;
    *total_bytes += chunk.len();
    if *chunk_count == 1 {
        runtime_proxy_log_to_path(
            &response.log_path,
            &runtime_proxy_structured_log_message(
                "first_local_chunk",
                [
                    runtime_proxy_log_field("request", response.request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("profile", response.profile_name.as_str()),
                    runtime_proxy_log_field("bytes", chunk.len().to_string()),
                    runtime_proxy_log_field(
                        "elapsed_ms",
                        started_at.elapsed().as_millis().to_string(),
                    ),
                ],
            ),
        );
        note_runtime_profile_latency_observation(
            &response.shared,
            &response.profile_name,
            RuntimeRouteKind::Responses,
            "ttfb",
            started_at.elapsed().as_millis() as u64,
        );
    }
    sink.write_chunk(chunk, flush_each_chunk || *chunk_count == 1)
}

fn runtime_streaming_error_log_value(error: &io::Error) -> String {
    redaction_redact_secret_like_text(&error.to_string()).replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_error_log_value_redacts_secret_like_material() {
        let error = io::Error::other(
            "stream failed\nAuthorization: Bearer stream-token\napi_key=stream-key",
        );
        let message = runtime_streaming_error_log_value(&error);

        assert!(!message.contains('\n'));
        assert!(message.contains("Authorization: Bearer <redacted>"));
        assert!(message.contains("api_key=<redacted>"));
        assert!(!message.contains("stream-token"));
        assert!(!message.contains("stream-key"));
    }
}
