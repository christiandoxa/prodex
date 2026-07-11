use super::*;
use redaction::redaction_redact_secret_like_text;
use runtime_proxy_crate::runtime_stream_response_should_flush_each_chunk;

pub(crate) fn write_runtime_streaming_response(
    writer: Box<dyn Write + Send + 'static>,
    mut response: RuntimeStreamingResponse,
) -> io::Result<()> {
    let mut writer = writer;
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
    let status = reqwest::StatusCode::from_u16(response.status)
        .ok()
        .and_then(|status| status.canonical_reason().map(str::to_string))
        .unwrap_or_else(|| "OK".to_string());
    write!(
        writer,
        "HTTP/1.1 {} {}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n",
        response.status, status
    )
    .map_err(|err| {
        log_writer_error("headers_start", 0, 0, &err);
        err
    })?;
    for (name, value) in &response.headers {
        write!(writer, "{name}: {value}\r\n").map_err(|err| {
            log_writer_error("header_line", 0, 0, &err);
            err
        })?;
    }
    writer.write_all(b"\r\n").inspect_err(|err| {
        log_writer_error("headers_end", 0, 0, err);
    })?;
    writer.flush().inspect_err(|err| {
        log_writer_error("headers_flush", 0, 0, err);
    })?;

    let mut buffer = [0_u8; 8192];
    let mut total_bytes = 0usize;
    let mut chunk_count = 0usize;
    let chunk_context = RuntimeStreamChunkContext {
        request_id: response.request_id,
        log_path: response.log_path.clone(),
        profile_name: response.profile_name.clone(),
        shared: response.shared.clone(),
        started_at: &started_at,
        log_writer_error: &log_writer_error,
        flush_each_chunk,
    };
    loop {
        let read = match response.body.read(&mut buffer) {
            Ok(read) => read,
            Err(err) => {
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
                            runtime_proxy_log_field(
                                "elapsed_ms",
                                started_at.elapsed().as_millis().to_string(),
                            ),
                            runtime_proxy_log_field(
                                "error",
                                runtime_streaming_error_log_value(&err),
                            ),
                        ],
                    ),
                );
                let transport_error =
                    anyhow::Error::new(io::Error::new(err.kind(), err.to_string()));
                if is_runtime_proxy_transport_failure(&transport_error) {
                    note_runtime_profile_latency_failure(
                        &response.shared,
                        &response.profile_name,
                        RuntimeRouteKind::Responses,
                        "stream_read_error",
                    );
                }
                return Err(err);
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
            let err = io::Error::new(
                io::ErrorKind::ConnectionReset,
                "injected runtime stream read failure",
            );
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
                        runtime_proxy_log_field(
                            "elapsed_ms",
                            started_at.elapsed().as_millis().to_string(),
                        ),
                        runtime_proxy_log_field("error", runtime_streaming_error_log_value(&err)),
                    ],
                ),
            );
            note_runtime_profile_latency_failure(
                &response.shared,
                &response.profile_name,
                RuntimeRouteKind::Responses,
                "stream_read_error",
            );
            return Err(err);
        }
        write_runtime_stream_chunk(
            &mut writer,
            &chunk_context,
            &buffer[..read],
            &mut chunk_count,
            &mut total_bytes,
        )?;
    }
    writer.write_all(b"0\r\n\r\n").inspect_err(|err| {
        log_writer_error("trailer", chunk_count, total_bytes, err);
    })?;
    writer.flush().inspect_err(|err| {
        log_writer_error("trailer_flush", chunk_count, total_bytes, err);
    })?;
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

fn runtime_streaming_error_log_value(err: &io::Error) -> String {
    redaction_redact_secret_like_text(&err.to_string()).replace('\n', " ")
}

struct RuntimeStreamChunkContext<'a> {
    request_id: u64,
    log_path: PathBuf,
    profile_name: String,
    shared: RuntimeRotationProxyShared,
    started_at: &'a Instant,
    log_writer_error: &'a dyn Fn(&str, usize, usize, &io::Error),
    flush_each_chunk: bool,
}

fn write_runtime_stream_chunk(
    writer: &mut Box<dyn Write + Send + 'static>,
    context: &RuntimeStreamChunkContext<'_>,
    chunk: &[u8],
    chunk_count: &mut usize,
    total_bytes: &mut usize,
) -> io::Result<()> {
    let RuntimeStreamChunkContext {
        request_id,
        log_path,
        profile_name,
        shared,
        started_at,
        log_writer_error,
        flush_each_chunk,
    } = context;
    if chunk.is_empty() {
        return Ok(());
    }

    *chunk_count += 1;
    *total_bytes += chunk.len();
    if *chunk_count == 1 {
        runtime_proxy_log_to_path(
            log_path,
            &runtime_proxy_structured_log_message(
                "first_local_chunk",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("profile", profile_name.as_str()),
                    runtime_proxy_log_field("bytes", chunk.len().to_string()),
                    runtime_proxy_log_field(
                        "elapsed_ms",
                        started_at.elapsed().as_millis().to_string(),
                    ),
                ],
            ),
        );
        note_runtime_profile_latency_observation(
            shared,
            profile_name,
            RuntimeRouteKind::Responses,
            "ttfb",
            started_at.elapsed().as_millis() as u64,
        );
    }
    write!(writer, "{:X}\r\n", chunk.len()).map_err(|err| {
        log_writer_error("chunk_size", *chunk_count, *total_bytes, &err);
        err
    })?;
    writer.write_all(chunk).inspect_err(|err| {
        log_writer_error("chunk_body", *chunk_count, *total_bytes, err);
    })?;
    writer.write_all(b"\r\n").inspect_err(|err| {
        log_writer_error("chunk_suffix", *chunk_count, *total_bytes, err);
    })?;
    if *flush_each_chunk || *chunk_count == 1 {
        writer.flush().inspect_err(|err| {
            log_writer_error("chunk_flush", *chunk_count, *total_bytes, err);
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_error_log_value_redacts_secret_like_material() {
        let err = io::Error::other(
            "stream failed\nAuthorization: Bearer stream-token\napi_key=stream-key",
        );
        let message = runtime_streaming_error_log_value(&err);

        assert!(!message.contains('\n'));
        assert!(message.contains("Authorization: Bearer <redacted>"));
        assert!(message.contains("api_key=<redacted>"));
        assert!(!message.contains("stream-token"));
        assert!(!message.contains("stream-key"));
    }
}
