use super::super::super::local_rewrite::RuntimeLocalRewriteAsyncResponse;
use super::super::super::local_rewrite_upstream::{
    RuntimeLocalRewriteLiveBody, RuntimeLocalRewriteLiveResponse,
    RuntimeLocalRewriteUpstreamResponse,
};
use super::super::super::provider_bridge::RuntimeProviderBridgeKind;
use super::*;
use prodex_provider_core::ProviderErrorClass;
use std::io::{Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

fn test_async_runtime() -> Arc<tokio::runtime::Runtime> {
    Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("SSE test runtime should build"),
    )
}

fn buffered(status: u16, body: &[u8]) -> RuntimeLocalRewriteUpstreamResult {
    RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Buffered(
            RuntimeHeapTrimmedBufferedResponseParts {
                status,
                headers: Vec::new(),
                body: body.to_vec().into(),
            },
        ),
        gemini_context: None,
        copilot_context: None,
    }
}

#[test]
fn upstream_error_log_value_is_content_free() {
    let error =
        anyhow::anyhow!("Bearer secret-sentinel for user@example.com in raw provider response");

    assert_eq!(
        runtime_local_rewrite_error_log_value(&error),
        "upstream_request_failed"
    );
}

#[test]
fn provider_fallback_requires_explicit_rate_limit_or_retryable_precommit_error() {
    assert_eq!(
        runtime_local_rewrite_provider_result_class(429),
        prodex_observability::ProviderResultClass::ProviderError,
    );
    assert_eq!(
        runtime_local_rewrite_provider_fallback_class(
            &buffered(429, b"too many requests"),
            RuntimeProviderBridgeKind::OpenAiResponses,
        ),
        None,
    );
    assert_eq!(
        runtime_local_rewrite_provider_fallback_class(
            &buffered(429, br#"{"error":{"code":"rate_limit_exceeded"}}"#),
            RuntimeProviderBridgeKind::OpenAiResponses,
        ),
        Some(ProviderErrorClass::RateLimit),
    );
    assert_eq!(
        runtime_local_rewrite_provider_fallback_class(
            &buffered(503, b"temporarily unavailable"),
            RuntimeProviderBridgeKind::OpenAiResponses,
        ),
        Some(ProviderErrorClass::Transient),
    );
}

fn live_sse(body: impl AsRef<[u8]> + Send + 'static) -> RuntimeLocalRewriteUpstreamResult {
    let body = body.as_ref().to_vec();
    live_sse_reader_with_length(Cursor::new(body.clone()), true, Some(body.len())).0
}

fn live_sse_reader_with_length(
    body: impl Read + Send + 'static,
    join_server: bool,
    content_length: Option<usize>,
) -> (RuntimeLocalRewriteUpstreamResult, Option<JoinHandle<()>>) {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("SSE test server should bind");
    let address = server
        .server_addr()
        .to_ip()
        .expect("SSE test server should expose an IP address");
    let sender = std::thread::spawn(move || {
        let request = server
            .recv()
            .expect("SSE test server should receive a request");
        let _ = request.respond(tiny_http::Response::new(
            tiny_http::StatusCode(200),
            vec![
                tiny_http::Header::from_bytes("content-type", "text/event-stream")
                    .expect("SSE content type header"),
            ],
            Box::new(body),
            content_length,
            None,
        ));
    });
    let async_runtime = test_async_runtime();
    let response = async_runtime
        .block_on(
            reqwest::Client::new()
                .get(format!("http://{address}"))
                .send(),
        )
        .expect("SSE test client should receive a response");
    let sender = if join_server {
        sender.join().expect("SSE test server should finish");
        None
    } else {
        Some(sender)
    };
    (
        RuntimeLocalRewriteUpstreamResult {
            response: RuntimeLocalRewriteUpstreamResponse::Live(
                RuntimeLocalRewriteLiveResponse::new(RuntimeLocalRewriteAsyncResponse::new(
                    response,
                    async_runtime,
                    crate::RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
                )),
            ),
            gemini_context: None,
            copilot_context: None,
        },
        sender,
    )
}

fn live_sse_raw(
    body: Vec<u8>,
    delayed_clean_end: Option<Duration>,
) -> (RuntimeLocalRewriteUpstreamResult, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("SSE test server should bind");
    let address = listener.local_addr().expect("SSE test address");
    let sender = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("SSE test connection");
        read_raw_request(&mut stream);
        if let Some(delay) = delayed_clean_end {
            stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
                    )
                    .unwrap();
            write!(stream, "{:x}\r\n", body.len()).unwrap();
            stream.write_all(&body).unwrap();
            stream.write_all(b"\r\n").unwrap();
            stream.flush().unwrap();
            std::thread::sleep(delay);
            stream.write_all(b"0\r\n\r\n").unwrap();
        } else {
            write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len() + 1
                )
                .unwrap();
            stream.write_all(&body).unwrap();
        }
        let _ = stream.flush();
    });
    let async_runtime = test_async_runtime();
    let response = async_runtime
        .block_on(
            reqwest::Client::new()
                .get(format!("http://{address}"))
                .send(),
        )
        .expect("SSE test client should receive a response");
    (
        RuntimeLocalRewriteUpstreamResult {
            response: RuntimeLocalRewriteUpstreamResponse::Live(
                RuntimeLocalRewriteLiveResponse::new(RuntimeLocalRewriteAsyncResponse::new(
                    response,
                    async_runtime,
                    crate::RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
                )),
            ),
            gemini_context: None,
            copilot_context: None,
        },
        sender,
    )
}

fn read_raw_request(stream: &mut TcpStream) {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 256];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = stream.read(&mut buffer).expect("SSE request should read");
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
    }
}

fn read_live_body(response: RuntimeLocalRewriteUpstreamResult) -> Vec<u8> {
    let RuntimeLocalRewriteUpstreamResponse::Live(mut live) = response.response else {
        panic!("SSE test response should remain live");
    };
    let mut body = live.prefix;
    live.body
        .take()
        .expect("SSE test body should remain available")
        .into_reader()
        .read_to_end(&mut body)
        .expect("SSE test response should remain readable");
    body
}

fn precommit_sse(response: &mut RuntimeLocalRewriteUpstreamResult, timeout_ms: u64) {
    let async_runtime = test_async_runtime();
    runtime_local_rewrite_precommit_live_provider_response(
        response,
        RuntimeProviderBridgeKind::DeepSeek,
        true,
        timeout_ms,
        crate::RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
        &async_runtime,
        &Arc::new(tokio::sync::Semaphore::new(1)),
    )
    .expect("SSE precommit lookahead should succeed");
}

#[test]
fn provider_sse_first_retryable_event_is_precommit_classified_for_chat_compatible_candidates() {
    // This guard intentionally covers only chat-compatible /v1/responses adapters.
    // Native provider protocols, including Anthropic Messages, stay outside this
    // precommit fallback contract until they have an explicit equivalent.
    let async_runtime = test_async_runtime();
    for (provider, body, expected) in [
        (
            RuntimeProviderBridgeKind::DeepSeek,
            concat!(r#"data: {"error":{"code":"insufficient_quota"}}"#, "\n\n"),
            ProviderErrorClass::Quota,
        ),
        (
            RuntimeProviderBridgeKind::Anthropic,
            concat!(r#"data: {"error":{"code":"rate_limit_exceeded"}}"#, "\n\n"),
            ProviderErrorClass::RateLimit,
        ),
        (
            RuntimeProviderBridgeKind::Gemini,
            concat!(r#"data: {"error":{"code":"server_is_overloaded"}}"#, "\n\n"),
            ProviderErrorClass::Transient,
        ),
    ] {
        let mut response = live_sse(body);
        runtime_local_rewrite_precommit_live_provider_response(
            &mut response,
            provider,
            true,
            crate::RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS,
            crate::RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
            &async_runtime,
            &Arc::new(tokio::sync::Semaphore::new(1)),
        )
        .expect("SSE precommit lookahead should succeed");
        assert_eq!(
            runtime_local_rewrite_provider_fallback_class(&response, provider),
            Some(expected)
        );
        let RuntimeLocalRewriteUpstreamResponse::Live(mut live) = response.response else {
            panic!("SSE test response should remain live");
        };
        let mut tail = Vec::new();
        let mut reader = live
            .body
            .take()
            .expect("SSE test body should remain available")
            .into_reader();
        std::io::Read::read_to_end(&mut reader, &mut tail)
            .expect("SSE test response should remain readable");
        let mut reconstructed = live.prefix;
        reconstructed.extend(tail);
        assert_eq!(reconstructed, body.as_bytes());
    }

    let body = concat!(
        r#"data: {"type":"response.output_text.delta","delta":"overloaded"}"#,
        "\n\n"
    );
    let mut response = live_sse(body);
    runtime_local_rewrite_precommit_live_provider_response(
        &mut response,
        RuntimeProviderBridgeKind::DeepSeek,
        true,
        crate::RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS,
        crate::RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
        &async_runtime,
        &Arc::new(tokio::sync::Semaphore::new(1)),
    )
    .expect("ordinary SSE lookahead should succeed");
    assert_eq!(
        runtime_local_rewrite_provider_fallback_class(
            &response,
            RuntimeProviderBridgeKind::DeepSeek,
        ),
        None
    );

    let mut response = live_sse(body);
    runtime_local_rewrite_precommit_live_provider_response(
        &mut response,
        RuntimeProviderBridgeKind::DeepSeek,
        false,
        crate::RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS,
        crate::RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
        &async_runtime,
        &Arc::new(tokio::sync::Semaphore::new(1)),
    )
    .expect("non-Responses SSE should remain untouched");
    let RuntimeLocalRewriteUpstreamResponse::Live(live) = response.response else {
        panic!("SSE test response should remain live");
    };
    assert!(live.prefix.is_empty());
}

#[test]
fn provider_sse_upstream_end_finalizes_partial_tail_for_retry_and_preserves_bytes() {
    let body = br#"data: {"error":{"code":"insufficient_quota"}}"#;
    let mut response = live_sse(body);

    precommit_sse(&mut response, crate::RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS);
    let RuntimeLocalRewriteUpstreamResponse::Live(live) = &response.response else {
        panic!("SSE test response should remain live");
    };
    assert!(live.upstream_eof);
    assert!(!live.headers.contains_key(reqwest::header::CONNECTION));
    assert_eq!(
        runtime_local_rewrite_provider_fallback_class(
            &response,
            RuntimeProviderBridgeKind::DeepSeek,
        ),
        Some(ProviderErrorClass::Quota)
    );

    assert_eq!(read_live_body(response), &body[..]);
}

#[test]
fn provider_sse_chunked_upstream_end_finalizes_partial_tail_at_true_eof() {
    let body = br#"data: {"error":{"code":"insufficient_quota"}}"#.to_vec();
    let expected = body.clone();
    let (mut response, sender) = live_sse_reader_with_length(Cursor::new(body), false, None);

    precommit_sse(&mut response, crate::RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS);

    let RuntimeLocalRewriteUpstreamResponse::Live(live) = &response.response else {
        panic!("SSE test response should remain live");
    };
    assert!(live.upstream_eof);
    assert_eq!(
        runtime_local_rewrite_provider_fallback_class(
            &response,
            RuntimeProviderBridgeKind::DeepSeek,
        ),
        Some(ProviderErrorClass::Quota)
    );
    assert_eq!(read_live_body(response), expected);
    sender
        .expect("SSE test server handle should exist")
        .join()
        .expect("SSE test server should finish");
}

#[test]
fn provider_sse_retry_stops_after_first_committed_event() {
    let body = concat!(
        r#"data: {"type":"response.output_text.delta","delta":"committed"}"#,
        "\n\n",
        r#"data: {"error":{"code":"insufficient_quota"}}"#,
        "\n\n",
    );
    let mut response = live_sse(body);

    precommit_sse(&mut response, crate::RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS);
    assert_eq!(
        runtime_local_rewrite_provider_fallback_class(
            &response,
            RuntimeProviderBridgeKind::DeepSeek,
        ),
        None
    );
    assert_eq!(read_live_body(response), body.as_bytes());
}

#[test]
fn provider_sse_budget_does_not_finalize_partial_tail_or_retry() {
    let mut body = br#"data: {"error":{"code":"insufficient_quota"}}"#.to_vec();
    body.resize(crate::RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES, b'x');
    let expected = body.clone();
    let mut response = live_sse(body);

    precommit_sse(&mut response, crate::RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS);
    assert_eq!(
        runtime_local_rewrite_provider_fallback_class(
            &response,
            RuntimeProviderBridgeKind::DeepSeek,
        ),
        None
    );

    assert_eq!(read_live_body(response), expected);
}

#[test]
fn provider_sse_timeout_does_not_finalize_partial_tail_or_retry() {
    let body = br#"data: {"error":{"code":"insufficient_quota"}}"#.to_vec();
    let expected = body.clone();
    let (mut response, sender) = live_sse_raw(body.clone(), Some(Duration::from_millis(100)));

    precommit_sse(&mut response, 10);

    assert_eq!(
        runtime_local_rewrite_provider_fallback_class(
            &response,
            RuntimeProviderBridgeKind::DeepSeek,
        ),
        None
    );
    let RuntimeLocalRewriteUpstreamResponse::Live(mut live) = response.response else {
        panic!("SSE test response should remain live");
    };
    let mut reconstructed = live.prefix;
    live.body
        .take()
        .expect("SSE test body should remain available")
        .into_reader()
        .read_to_end(&mut reconstructed)
        .expect("clean upstream EOF should remain a clean EOF");
    assert_eq!(reconstructed, expected);
    sender.join().expect("SSE test server should finish");
}

#[test]
fn provider_sse_channel_error_does_not_finalize_partial_tail_or_retry() {
    let body = br#"data: {"error":{"code":"insufficient_quota"}}"#.to_vec();
    let expected = body.clone();
    let (mut response, sender) = live_sse_raw(body.clone(), None);

    precommit_sse(&mut response, crate::RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS);

    assert_eq!(
        runtime_local_rewrite_provider_fallback_class(
            &response,
            RuntimeProviderBridgeKind::DeepSeek,
        ),
        None
    );
    let RuntimeLocalRewriteUpstreamResponse::Live(mut live) = response.response else {
        panic!("SSE test response should remain live");
    };
    let mut reconstructed = live.prefix;
    let _error = live
        .body
        .take()
        .expect("SSE test body should remain available")
        .into_reader()
        .read_to_end(&mut reconstructed)
        .expect_err("SSE channel error should remain visible");
    assert_eq!(reconstructed, expected);
    sender.join().expect("SSE test server should finish");
}

#[test]
fn provider_sse_prefetch_saturation_preserves_the_original_live_body() {
    let async_runtime = test_async_runtime();
    let mut response = live_sse("data: {}\n\n");

    runtime_local_rewrite_precommit_live_provider_response(
        &mut response,
        RuntimeProviderBridgeKind::DeepSeek,
        true,
        crate::RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS,
        crate::RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
        &async_runtime,
        &Arc::new(tokio::sync::Semaphore::new(0)),
    )
    .expect("saturated lookahead should pass through");

    let RuntimeLocalRewriteUpstreamResponse::Live(live) = response.response else {
        panic!("SSE test response should remain live");
    };
    assert!(live.prefix.is_empty());
    assert!(matches!(
        live.body,
        Some(RuntimeLocalRewriteLiveBody::AsyncResponse(_))
    ));
}
