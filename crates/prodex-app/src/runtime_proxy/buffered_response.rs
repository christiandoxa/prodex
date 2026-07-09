use anyhow::{Context, Result};
use std::io::{self, Cursor, Read};

use crate::{
    RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, TinyHeader, TinyResponse, TinyStatusCode,
    runtime_maybe_trim_process_heap,
};
use runtime_proxy_crate::runtime_forward_binary_response_headers;

pub(crate) fn build_runtime_proxy_text_response_parts(
    status: u16,
    message: &str,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    RuntimeHeapTrimmedBufferedResponseParts::from_crate_parts(
        runtime_proxy_crate::build_runtime_proxy_text_response_parts(status, message),
    )
}

pub(crate) fn build_runtime_proxy_text_response(
    status: u16,
    message: &str,
) -> tiny_http::ResponseBox {
    build_runtime_proxy_response_from_parts(build_runtime_proxy_text_response_parts(
        status, message,
    ))
}

pub(crate) fn build_runtime_proxy_json_error_parts(
    status: u16,
    code: &str,
    message: &str,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    RuntimeHeapTrimmedBufferedResponseParts::from_crate_parts(
        runtime_proxy_crate::build_runtime_proxy_json_error_parts(status, code, message),
    )
}

pub(crate) fn build_runtime_proxy_json_error_response(
    status: u16,
    code: &str,
    message: &str,
) -> tiny_http::ResponseBox {
    build_runtime_proxy_response_from_parts(build_runtime_proxy_json_error_parts(
        status, code, message,
    ))
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeHeapTrimmedResponseBody {
    bytes: Vec<u8>,
}

impl RuntimeHeapTrimmedResponseBody {
    pub(crate) fn into_vec(mut self) -> Vec<u8> {
        std::mem::take(&mut self.bytes)
    }
}

impl From<Vec<u8>> for RuntimeHeapTrimmedResponseBody {
    fn from(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

impl std::ops::Deref for RuntimeHeapTrimmedResponseBody {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

impl<'a> IntoIterator for &'a RuntimeHeapTrimmedResponseBody {
    type Item = &'a u8;
    type IntoIter = std::slice::Iter<'a, u8>;

    fn into_iter(self) -> Self::IntoIter {
        self.bytes.iter()
    }
}

impl Drop for RuntimeHeapTrimmedResponseBody {
    fn drop(&mut self) {
        let released_bytes = std::mem::take(&mut self.bytes).capacity();
        let _ = runtime_maybe_trim_process_heap(released_bytes);
    }
}

pub(crate) struct RuntimeHeapTrimmedBufferedResponseParts {
    pub(crate) status: u16,
    pub(crate) headers: Vec<(String, Vec<u8>)>,
    pub(crate) body: RuntimeHeapTrimmedResponseBody,
}

impl RuntimeHeapTrimmedBufferedResponseParts {
    pub(crate) fn from_crate_parts(
        parts: runtime_proxy_crate::RuntimeBufferedResponseParts,
    ) -> Self {
        Self {
            status: parts.status,
            headers: parts.headers,
            body: parts.body.into_vec().into(),
        }
    }
}

pub(crate) fn read_runtime_buffered_response_body_with_limit(
    reader: impl Read,
    max_bytes: usize,
    context: &str,
) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    let mut reader = reader.take((max_bytes as u64).saturating_add(1));
    reader
        .read_to_end(&mut body)
        .with_context(|| context.to_string())?;
    if body.len() > max_bytes {
        return Err(buffered_response_limit_error(max_bytes));
    }
    Ok(body)
}

fn buffered_response_limit_error(max_bytes: usize) -> anyhow::Error {
    anyhow::Error::new(io::Error::new(
        io::ErrorKind::InvalidData,
        format!("buffered response exceeded safe size limit ({})", max_bytes),
    ))
}

pub(crate) fn read_blocking_response_body_with_limit(
    response: reqwest::blocking::Response,
    max_bytes: usize,
    context: &str,
) -> Result<Vec<u8>> {
    read_runtime_buffered_response_body_with_limit(response, max_bytes, context)
}

pub(crate) fn read_blocking_response_text_with_limit(
    response: reqwest::blocking::Response,
    max_bytes: usize,
    context: &str,
) -> Result<String> {
    let body = read_blocking_response_body_with_limit(response, max_bytes, context)?;
    Ok(String::from_utf8_lossy(&body).into_owned())
}

pub(crate) async fn read_async_response_body_with_limit(
    mut response: reqwest::Response,
    max_bytes: usize,
    context: &str,
) -> Result<Vec<u8>> {
    read_async_response_body_chunks_with_limit(&mut response, Vec::new(), max_bytes, context).await
}

async fn read_async_response_body_chunks_with_limit(
    response: &mut reqwest::Response,
    mut body: Vec<u8>,
    max_bytes: usize,
    context: &str,
) -> Result<Vec<u8>> {
    loop {
        let next = response
            .chunk()
            .await
            .with_context(|| context.to_string())?;
        let Some(chunk) = next else {
            break;
        };
        if body.len().saturating_add(chunk.len()) > max_bytes {
            return Err(buffered_response_limit_error(max_bytes));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[derive(Debug)]
struct RuntimeBufferedResponseBodyReader {
    cursor: Cursor<Vec<u8>>,
}

impl RuntimeBufferedResponseBodyReader {
    fn new(body: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(body),
        }
    }
}

impl Read for RuntimeBufferedResponseBodyReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.cursor.read(buf)
    }
}

impl Drop for RuntimeBufferedResponseBodyReader {
    fn drop(&mut self) {
        let released_bytes = std::mem::take(self.cursor.get_mut()).capacity();
        let _ = runtime_maybe_trim_process_heap(released_bytes);
    }
}

pub(crate) async fn buffer_runtime_proxy_async_response_parts(
    response: reqwest::Response,
    prelude: Vec<u8>,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    buffer_runtime_proxy_async_response_parts_with_limit(
        response,
        prelude,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
    )
    .await
}

pub(crate) async fn buffer_runtime_proxy_async_response_parts_with_limit(
    mut response: reqwest::Response,
    prelude: Vec<u8>,
    max_bytes: usize,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let status = response.status().as_u16();
    let headers = runtime_forward_binary_response_headers(
        response
            .headers()
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_bytes())),
    );
    let body = read_async_response_body_chunks_with_limit(
        &mut response,
        prelude,
        max_bytes,
        "failed to read upstream runtime response body chunk",
    )
    .await?;
    Ok(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers,
        body: body.into(),
    })
}

pub(crate) fn build_runtime_proxy_response_from_parts(
    parts: RuntimeHeapTrimmedBufferedResponseParts,
) -> tiny_http::ResponseBox {
    let status = TinyStatusCode(parts.status);
    let headers = parts
        .headers
        .into_iter()
        .filter_map(|(name, value)| TinyHeader::from_bytes(name.as_bytes(), value).ok())
        .collect::<Vec<_>>();
    let body_len = parts.body.len();
    TinyResponse::new(
        status,
        headers,
        Box::new(RuntimeBufferedResponseBodyReader::new(
            parts.body.into_vec(),
        )),
        Some(body_len),
        None,
    )
    .boxed()
}

pub(crate) fn runtime_buffered_response_content_type(
    parts: &RuntimeHeapTrimmedBufferedResponseParts,
) -> Option<&str> {
    runtime_proxy_crate::runtime_response_content_type_from_binary_headers(
        parts
            .headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_slice())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use tiny_http::{Response as TinyResponse, Server as TinyServer};

    #[test]
    fn buffered_response_body_limit_allows_exact_limit() {
        let body = vec![b'a'; 4];

        let captured =
            read_runtime_buffered_response_body_with_limit(Cursor::new(body.clone()), 4, "read")
                .unwrap();

        assert_eq!(captured, body);
    }

    #[test]
    fn buffered_response_body_limit_rejects_limit_plus_one() {
        let err =
            read_runtime_buffered_response_body_with_limit(Cursor::new(vec![b'a'; 5]), 4, "read")
                .expect_err("body above limit should be rejected");

        assert!(err.to_string().contains("safe size limit"));
    }

    #[test]
    fn buffered_response_text_limit_rejects_limit_plus_one() {
        let server = TinyServer::http("127.0.0.1:0").expect("test server should bind");
        let addr = server.server_addr().to_ip().unwrap();
        let handle = thread::spawn(move || {
            let request = server.recv().expect("request should arrive");
            request
                .respond(TinyResponse::from_data(vec![b'a'; 5]))
                .expect("response should send");
        });

        let response =
            reqwest::blocking::get(format!("http://{addr}/")).expect("response should be received");
        let err = read_blocking_response_text_with_limit(response, 4, "read")
            .expect_err("body above limit should be rejected");
        handle.join().expect("test server should finish");

        assert!(err.to_string().contains("safe size limit"));
    }

    #[test]
    fn buffered_async_response_strips_standard_hop_by_hop_headers() {
        let server = TinyServer::http("127.0.0.1:0").expect("test server should bind");
        let addr = server.server_addr().to_ip().unwrap();
        let handle = thread::spawn(move || {
            let request = server.recv().expect("request should arrive");
            let response = TinyResponse::from_string("ok")
                .with_header(TinyHeader::from_bytes("Server", "upstream").unwrap())
                .with_header(TinyHeader::from_bytes("x-codex-turn-state", "keep-me").unwrap());
            request.respond(response).expect("response should send");
        });

        let parts = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
            .block_on(async {
                let response = reqwest::get(format!("http://{addr}/"))
                    .await
                    .expect("response should be received");
                buffer_runtime_proxy_async_response_parts(response, Vec::new())
                    .await
                    .expect("response should buffer")
            });
        handle.join().expect("test server should finish");

        assert!(
            !parts.headers.iter().any(|(name, _)| name == "server"),
            "buffered response must not forward hop-by-hop/runtime-local headers"
        );
        assert!(parts.headers.iter().any(|(name, value)| {
            name == "x-codex-turn-state" && value.as_slice() == b"keep-me"
        }));
    }
}
