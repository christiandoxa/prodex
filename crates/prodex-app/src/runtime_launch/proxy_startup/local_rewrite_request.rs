use prodex_mojo_core::rich::ascii_casefold_equal_exact;
use std::{io, time::Instant};

use prodex_observability::{ApiRouteKind, ApiStatusClass};

use super::local_rewrite_response::runtime_local_rewrite_append_call_id_header;
use crate::{
    RuntimeCapturedRequest, RuntimeProxyRequest, RuntimeStreamingResponse,
    capture_runtime_proxy_request,
};

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
            ascii_casefold_equal_exact(name, "upgrade")
                .expect("Mojo upgrade header-name comparison failed")
                && ascii_casefold_equal_exact(value, "websocket")
                    .expect("Mojo websocket upgrade-value comparison failed")
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

    pub(super) fn capture(
        &mut self,
        max_body_bytes: u64,
    ) -> anyhow::Result<RuntimeCapturedRequest<'static>> {
        capture_runtime_proxy_request(&mut self.request, max_body_bytes)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    #[test]
    fn capture_enforces_explicit_limit_for_declared_and_chunked_bodies() {
        crate::runtime_request_memory::initialize_request_memory_budget();
        for (framing, payload) in [
            ("Content-Length: 4\r\n", "abcd"),
            ("Content-Length: 5\r\n", "abcde"),
            ("Transfer-Encoding: chunked\r\n", "4\r\nabcd\r\n0\r\n\r\n"),
            ("Transfer-Encoding: chunked\r\n", "5\r\nabcde\r\n0\r\n\r\n"),
        ] {
            let server = tiny_http::Server::http("127.0.0.1:0").expect("server");
            let addr = server.server_addr().to_ip().expect("address");
            let client = std::thread::spawn(move || {
                let mut stream = TcpStream::connect(addr).expect("connect");
                write!(stream, "POST /v1/responses?x=1 HTTP/1.1\r\nHost: localhost\r\nX-Test: preserved\r\n{framing}Connection: close\r\n\r\n{payload}")
                    .expect("send");
                let mut response = Vec::new();
                let _ = stream.read_to_end(&mut response);
            });
            let mut request = RuntimeLocalRewriteRequest::tiny(server.recv().expect("request"));
            let result = request.capture(4);
            if payload.contains("abcde") {
                assert!(crate::runtime_proxy::runtime_proxy_error_is_body_too_large(
                    &result.err().expect("oversized body")
                ));
            } else {
                let captured = result.expect("body at limit").request;
                assert_eq!(captured.body, b"abcd");
                assert_eq!(captured.method, "POST");
                assert_eq!(captured.path_and_query, "/v1/responses?x=1");
                assert!(captured.headers.iter().any(|(name, value)| {
                    name.eq_ignore_ascii_case("x-test") && value == "preserved"
                }));
            }
            request
                .respond(tiny_http::Response::empty(200).boxed())
                .expect("respond");
            client.join().expect("client");
        }
    }
}
