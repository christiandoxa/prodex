use anyhow::{Context, Result};
use std::io::{self, Cursor, Read};

use crate::{
    RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, TinyHeader, TinyResponse, TinyStatusCode,
    runtime_maybe_trim_process_heap,
};
use runtime_proxy_crate::should_skip_runtime_response_header;

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
    let mut headers = Vec::new();
    for (name, value) in response.headers() {
        if should_skip_runtime_response_header(name.as_str()) {
            continue;
        }
        headers.push((name.as_str().to_string(), value.as_bytes().to_vec()));
    }
    let mut body = prelude;
    loop {
        let next = response
            .chunk()
            .await
            .context("failed to read upstream runtime response body chunk")?;
        let Some(chunk) = next else {
            break;
        };
        if body.len().saturating_add(chunk.len()) > max_bytes {
            return Err(anyhow::Error::new(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "runtime buffered response exceeded safe size limit ({})",
                    max_bytes
                ),
            )));
        }
        body.extend_from_slice(&chunk);
    }
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
