use super::*;

pub(crate) fn build_runtime_proxy_text_response_parts(
    status: u16,
    message: &str,
) -> RuntimeBufferedResponseParts {
    RuntimeBufferedResponseParts {
        status,
        headers: vec![(
            "Content-Type".to_string(),
            b"text/plain; charset=utf-8".to_vec(),
        )],
        body: message.as_bytes().to_vec().into(),
    }
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
) -> RuntimeBufferedResponseParts {
    let body = serde_json::json!({
        "error": {
            "code": code,
            "message": message,
        }
    })
    .to_string();
    RuntimeBufferedResponseParts {
        status,
        headers: vec![("Content-Type".to_string(), b"application/json".to_vec())],
        body: body.into_bytes().into(),
    }
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
pub(crate) struct RuntimeManagedResponseBody {
    bytes: Vec<u8>,
}

impl RuntimeManagedResponseBody {
    pub(crate) fn into_vec(mut self) -> Vec<u8> {
        std::mem::take(&mut self.bytes)
    }
}

impl From<Vec<u8>> for RuntimeManagedResponseBody {
    fn from(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

impl std::ops::Deref for RuntimeManagedResponseBody {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

impl<'a> IntoIterator for &'a RuntimeManagedResponseBody {
    type Item = &'a u8;
    type IntoIter = std::slice::Iter<'a, u8>;

    fn into_iter(self) -> Self::IntoIter {
        self.bytes.iter()
    }
}

impl Drop for RuntimeManagedResponseBody {
    fn drop(&mut self) {
        let released_bytes = std::mem::take(&mut self.bytes).capacity();
        let _ = runtime_maybe_trim_process_heap(released_bytes);
    }
}

pub(crate) struct RuntimeBufferedResponseParts {
    pub(crate) status: u16,
    pub(crate) headers: Vec<(String, Vec<u8>)>,
    pub(crate) body: RuntimeManagedResponseBody,
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

pub(crate) fn buffer_runtime_proxy_async_response_parts(
    shared: &RuntimeRotationProxyShared,
    response: reqwest::Response,
    prelude: Vec<u8>,
) -> Result<RuntimeBufferedResponseParts> {
    buffer_runtime_proxy_async_response_parts_with_limit(
        shared,
        response,
        prelude,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
    )
}

pub(crate) fn buffer_runtime_proxy_async_response_parts_with_limit(
    shared: &RuntimeRotationProxyShared,
    mut response: reqwest::Response,
    prelude: Vec<u8>,
    max_bytes: usize,
) -> Result<RuntimeBufferedResponseParts> {
    let status = response.status().as_u16();
    let mut headers = Vec::new();
    for (name, value) in response.headers() {
        if should_skip_runtime_response_header(name.as_str()) {
            continue;
        }
        headers.push((name.as_str().to_string(), value.as_bytes().to_vec()));
    }
    let body = shared.async_runtime.block_on(async move {
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
        Ok::<Vec<u8>, anyhow::Error>(body)
    })?;
    Ok(RuntimeBufferedResponseParts {
        status,
        headers,
        body: body.into(),
    })
}

pub(crate) fn build_runtime_proxy_response_from_parts(
    parts: RuntimeBufferedResponseParts,
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
    parts: &RuntimeBufferedResponseParts,
) -> Option<&str> {
    parts.headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("content-type")
            .then(|| std::str::from_utf8(value).ok())
            .flatten()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}
