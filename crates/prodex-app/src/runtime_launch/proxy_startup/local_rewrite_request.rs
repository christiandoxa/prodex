use std::{
    io::{self, Read},
    time::Instant,
};

use prodex_observability::{ApiRouteKind, ApiStatusClass};

use super::local_rewrite_response::runtime_local_rewrite_append_call_id_header;
use crate::{RuntimeProxyBodyTooLarge, RuntimeProxyRequest, RuntimeStreamingResponse};

pub(super) struct RuntimeLocalRewriteRequest {
    method: String,
    path_and_query: String,
    headers: Vec<(String, String)>,
    metric_started_at: Instant,
    request: tiny_http::Request,
}

impl RuntimeLocalRewriteRequest {
    pub(super) fn tiny(request: tiny_http::Request) -> Self {
        let method = request.method().as_str().to_string();
        let path_and_query = request.url().to_string();
        let headers = request
            .headers()
            .iter()
            .map(|header| {
                (
                    header.field.as_str().as_str().to_string(),
                    header.value.as_str().to_string(),
                )
            })
            .collect();
        Self {
            method,
            path_and_query,
            headers,
            metric_started_at: Instant::now(),
            request,
        }
    }

    pub(super) fn url(&self) -> &str {
        &self.path_and_query
    }

    pub(super) fn is_websocket_upgrade(&self) -> bool {
        self.headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("upgrade") && value.eq_ignore_ascii_case("websocket")
        })
    }

    pub(super) fn header_request(&self) -> RuntimeProxyRequest {
        RuntimeProxyRequest {
            method: self.method.clone(),
            path_and_query: self.path_and_query.clone(),
            headers: self.headers.clone(),
            body: Vec::new(),
        }
    }

    pub(super) fn capture(&mut self, max_body_bytes: u64) -> anyhow::Result<RuntimeProxyRequest> {
        if let Some(content_length) = self.content_length()
            && content_length > max_body_bytes
        {
            return Err(RuntimeProxyBodyTooLarge::new(max_body_bytes, Some(content_length)).into());
        }
        let mut body = Vec::new();
        self.request
            .as_reader()
            .take(max_body_bytes.saturating_add(1))
            .read_to_end(&mut body)?;
        if body.len() as u64 > max_body_bytes {
            return Err(RuntimeProxyBodyTooLarge::new(max_body_bytes, None).into());
        }
        Ok(RuntimeProxyRequest {
            method: self.method.clone(),
            path_and_query: self.path_and_query.clone(),
            headers: self.headers.clone(),
            body,
        })
    }

    pub(super) fn respond(self, response: tiny_http::ResponseBox) -> io::Result<()> {
        self.record_api_red_metric(response.status_code().0);
        self.request.respond(response)
    }

    pub(super) fn stream(
        self,
        mut response: RuntimeStreamingResponse,
        call_id_shared: Option<&super::local_rewrite::RuntimeLocalRewriteProxyShared>,
    ) -> io::Result<()> {
        self.record_api_red_metric(response.status);
        if let Some(shared) = call_id_shared {
            runtime_local_rewrite_append_call_id_header(
                &mut response.headers,
                response.request_id,
                shared,
            );
        }
        crate::write_runtime_streaming_response(self.request.into_writer(), response)
    }

    fn content_length(&self) -> Option<u64> {
        self.headers.iter().find_map(|(name, value)| {
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())
                .flatten()
        })
    }

    fn record_api_red_metric(&self, status: u16) {
        let route = runtime_api_route_kind(&self.path_and_query, self.is_websocket_upgrade());
        let status_class = runtime_api_status_class(status);
        let duration_ms =
            u64::try_from(self.metric_started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        crate::runtime_operational_metrics::record_runtime_api_red_metric(
            route,
            status_class,
            duration_ms,
        );
    }
}

pub(super) fn runtime_api_route_kind(path_and_query: &str, websocket: bool) -> ApiRouteKind {
    let path = path_and_query.split('?').next().unwrap_or(path_and_query);
    if websocket {
        ApiRouteKind::Websocket
    } else if path.ends_with("/responses/compact") {
        ApiRouteKind::Compact
    } else if matches!(
        path,
        "/healthz" | "/readyz" | "/livez" | "/startupz" | "/metrics"
    ) {
        ApiRouteKind::Health
    } else {
        ApiRouteKind::Responses
    }
}

fn runtime_api_status_class(status: u16) -> ApiStatusClass {
    match status {
        100..=199 => ApiStatusClass::Informational,
        200..=299 => ApiStatusClass::Success,
        300..=399 => ApiStatusClass::Redirection,
        400..=499 => ApiStatusClass::ClientError,
        _ => ApiStatusClass::ServerError,
    }
}

pub(super) fn runtime_local_rewrite_request_target_valid(raw: &str) -> bool {
    if raw.is_empty() || raw.len() > 8 * 1024 || !raw.is_ascii() {
        return false;
    }
    if raw.bytes().any(|byte| byte <= b' ' || byte == 0x7f) {
        return false;
    }
    if !raw.starts_with('/') || raw.starts_with("//") || raw.contains('#') {
        return false;
    }
    let path_len = raw.find('?').unwrap_or(raw.len());
    let path = &raw[..path_len];
    if path.contains('\\')
        || path.contains("//")
        || path.split('/').any(|s| matches!(s, "." | ".."))
    {
        return false;
    }
    let bytes = raw.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            index += 1;
            continue;
        }
        let Some(encoded) = bytes.get(index + 1..index + 3) else {
            return false;
        };
        let Some(high) = hex_value(encoded[0]) else {
            return false;
        };
        let Some(low) = hex_value(encoded[1]) else {
            return false;
        };
        let decoded = (high << 4) | low;
        if index < path_len
            && (decoded >= 0x80
                || decoded <= b' '
                || decoded == 0x7f
                || matches!(decoded, b'/' | b'\\' | b'%' | b'?' | b'#' | b'.'))
        {
            return false;
        }
        index += 3;
    }
    true
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
