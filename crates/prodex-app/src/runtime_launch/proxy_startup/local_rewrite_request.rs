use std::{
    io::{self, Read},
    num::NonZeroUsize,
    sync::Arc,
};

use bytes::Bytes;
use prodex_gateway_server::{
    GatewayHandlerError, GatewayHandlerResponse, GatewayHandlerResult, GatewayResponseBodySender,
    bounded_in_process_upgrade, bounded_response_body_with_guard,
};
use tokio::sync::{mpsc, oneshot};

use super::local_rewrite_response::runtime_local_rewrite_append_call_id_header;
use crate::{
    RuntimeProxyBodyTooLarge, RuntimeProxyRequest, RuntimeStreamingResponse, TinyReadWrite,
    write_runtime_gateway_streaming_response,
};

const DIRECT_BODY_CHANNEL_CAPACITY: NonZeroUsize = NonZeroUsize::new(2).unwrap();
const DIRECT_UPGRADE_CHANNEL_CAPACITY: NonZeroUsize = NonZeroUsize::new(8).unwrap();

pub(super) struct RuntimeLocalRewriteRequest {
    method: String,
    path_and_query: String,
    headers: Vec<(String, String)>,
    network_zone: prodex_domain::NetworkZone,
    mtls_peer_certificate_sha256: Option<[u8; 32]>,
    transport: RuntimeLocalRewriteTransport,
}

enum RuntimeLocalRewriteTransport {
    Tiny(tiny_http::Request),
    Direct {
        body: RuntimeDirectRequestBody,
        reply: RuntimeDirectReply,
    },
}

pub(super) struct RuntimeDirectRequestBodySender {
    sender: mpsc::Sender<RuntimeDirectRequestBodyMessage>,
}

pub(super) struct RuntimeDirectRequestBody {
    receiver: mpsc::Receiver<RuntimeDirectRequestBodyMessage>,
    current: Bytes,
    offset: usize,
    ended: bool,
}

enum RuntimeDirectRequestBodyMessage {
    Chunk(Bytes),
    End,
    Error(io::Error),
}

pub(super) struct RuntimeDirectReply {
    head: Option<oneshot::Sender<GatewayHandlerResult>>,
    permit: Arc<tokio::sync::OwnedSemaphorePermit>,
}

impl RuntimeLocalRewriteRequest {
    pub(super) fn tiny(request: tiny_http::Request) -> Self {
        let network_zone =
            request
                .remote_addr()
                .map_or(prodex_domain::NetworkZone::Unknown, |peer| {
                    if peer.ip().is_loopback() {
                        prodex_domain::NetworkZone::Local
                    } else {
                        prodex_domain::NetworkZone::Unknown
                    }
                });
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
            network_zone,
            mtls_peer_certificate_sha256: None,
            transport: RuntimeLocalRewriteTransport::Tiny(request),
        }
    }

    pub(super) fn direct(
        method: String,
        path_and_query: String,
        headers: Vec<(String, String)>,
        network_zone: prodex_domain::NetworkZone,
        mtls_peer_certificate_sha256: Option<[u8; 32]>,
        body: RuntimeDirectRequestBody,
        reply: RuntimeDirectReply,
    ) -> Self {
        Self {
            method,
            path_and_query,
            headers,
            network_zone,
            mtls_peer_certificate_sha256,
            transport: RuntimeLocalRewriteTransport::Direct { body, reply },
        }
    }

    pub(super) fn method(&self) -> &str {
        &self.method
    }

    pub(super) fn url(&self) -> &str {
        &self.path_and_query
    }

    pub(super) fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    pub(super) fn network_zone(&self) -> prodex_domain::NetworkZone {
        self.network_zone
    }

    pub(super) fn mtls_peer_certificate_sha256(&self) -> Option<[u8; 32]> {
        self.mtls_peer_certificate_sha256
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
        self.body_reader()
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

    pub(super) fn capture_with_limit(
        &mut self,
        configured_max_body_bytes: u64,
        limit: u64,
    ) -> anyhow::Result<RuntimeProxyRequest> {
        self.capture(configured_max_body_bytes.min(limit))
    }

    pub(super) fn respond(self, response: tiny_http::ResponseBox) -> io::Result<()> {
        match self.transport {
            RuntimeLocalRewriteTransport::Tiny(request) => request.respond(response),
            RuntimeLocalRewriteTransport::Direct { reply, .. } => reply.respond(response),
        }
    }

    pub(super) fn stream(
        self,
        mut response: RuntimeStreamingResponse,
        call_id_shared: Option<&super::local_rewrite::RuntimeLocalRewriteProxyShared>,
    ) -> io::Result<()> {
        if let Some(shared) = call_id_shared {
            runtime_local_rewrite_append_call_id_header(
                &mut response.headers,
                response.request_id,
                shared,
            );
        }
        match self.transport {
            RuntimeLocalRewriteTransport::Tiny(request) => {
                crate::write_runtime_streaming_response(request.into_writer(), response)
            }
            RuntimeLocalRewriteTransport::Direct { reply, .. } => reply.stream(response),
        }
    }

    pub(super) fn upgrade(
        self,
        protocol: &str,
        response: tiny_http::ResponseBox,
    ) -> io::Result<Box<dyn TinyReadWrite + Send>> {
        match self.transport {
            RuntimeLocalRewriteTransport::Tiny(request) => Ok(request.upgrade(protocol, response)),
            RuntimeLocalRewriteTransport::Direct { reply, .. } => reply.upgrade(protocol, response),
        }
    }

    fn content_length(&self) -> Option<u64> {
        self.headers.iter().find_map(|(name, value)| {
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())
                .flatten()
        })
    }

    fn body_reader(&mut self) -> &mut dyn Read {
        match &mut self.transport {
            RuntimeLocalRewriteTransport::Tiny(request) => request.as_reader(),
            RuntimeLocalRewriteTransport::Direct { body, .. } => body,
        }
    }
}

impl RuntimeDirectReply {
    pub(super) fn new(
        head: oneshot::Sender<GatewayHandlerResult>,
        permit: Arc<tokio::sync::OwnedSemaphorePermit>,
    ) -> Self {
        Self {
            head: Some(head),
            permit,
        }
    }

    fn respond(mut self, response: tiny_http::ResponseBox) -> io::Result<()> {
        let status = response.status_code().0;
        let headers = tiny_response_headers(&response);
        let content_length = response.data_length();
        let mut reader = response.into_reader();
        let (body_sender, body) = bounded_response_body_with_guard(
            DIRECT_BODY_CHANNEL_CAPACITY,
            Arc::clone(&self.permit),
        );
        let head = prodex_gateway_server::GatewayHandlerResponse::from_parts(
            status,
            headers,
            content_length,
            body,
        );
        self.send_head(head)?;
        copy_response_body(&mut reader, body_sender)
    }

    fn stream(mut self, response: RuntimeStreamingResponse) -> io::Result<()> {
        let headers = response
            .headers
            .iter()
            .map(|(name, value)| (name.clone(), value.as_bytes().to_vec()))
            .collect();
        let (body_sender, body) = bounded_response_body_with_guard(
            DIRECT_BODY_CHANNEL_CAPACITY,
            Arc::clone(&self.permit),
        );
        let head = GatewayHandlerResponse::from_parts(response.status, headers, None, body);
        self.send_head(head)?;
        write_runtime_gateway_streaming_response(body_sender, response)
    }

    fn upgrade(
        mut self,
        protocol: &str,
        response: tiny_http::ResponseBox,
    ) -> io::Result<Box<dyn TinyReadWrite + Send>> {
        let status = response.status_code().0;
        let mut headers = tiny_response_headers(&response);
        append_header_if_missing(&mut headers, "Connection", b"Upgrade");
        append_header_if_missing(&mut headers, "Upgrade", protocol.as_bytes());
        let (sender, body) = bounded_response_body_with_guard(
            DIRECT_BODY_CHANNEL_CAPACITY,
            Arc::clone(&self.permit),
        );
        sender.finish()?;
        let (application, handoff) = bounded_in_process_upgrade(DIRECT_UPGRADE_CHANNEL_CAPACITY);
        let head = GatewayHandlerResponse::from_parts(status, headers, None, body).map(|head| {
            head.with_in_process_upgrade_handoff(handoff.with_guard(Arc::clone(&self.permit)))
        });
        self.send_head(head)?;
        Ok(Box::new(application))
    }

    fn send_head(&mut self, head: GatewayHandlerResult) -> io::Result<()> {
        self.head
            .take()
            .ok_or_else(|| io::Error::other("gateway response head already sent"))?
            .send(head)
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "gateway request closed"))
    }
}

impl Drop for RuntimeDirectReply {
    fn drop(&mut self) {
        if let Some(head) = self.head.take() {
            let _ = head.send(Err(GatewayHandlerError::Unavailable));
        }
    }
}

impl Read for RuntimeDirectRequestBody {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.ended {
            return Ok(0);
        }
        while self.offset == self.current.len() {
            match self.receiver.blocking_recv() {
                Some(RuntimeDirectRequestBodyMessage::Chunk(chunk)) => {
                    self.current = chunk;
                    self.offset = 0;
                }
                Some(RuntimeDirectRequestBodyMessage::End) => {
                    self.ended = true;
                    return Ok(0);
                }
                Some(RuntimeDirectRequestBodyMessage::Error(error)) => return Err(error),
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "gateway request body ended without an end marker",
                    ));
                }
            };
        }
        let read = buffer.len().min(self.current.len() - self.offset);
        buffer[..read].copy_from_slice(&self.current[self.offset..self.offset + read]);
        self.offset += read;
        Ok(read)
    }
}

impl RuntimeDirectRequestBodySender {
    pub(super) async fn send(&self, bytes: Bytes) -> io::Result<()> {
        self.sender
            .send(RuntimeDirectRequestBodyMessage::Chunk(bytes))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "gateway request body closed"))
    }

    pub(super) async fn send_error(self, error: io::Error) {
        let _ = self
            .sender
            .send(RuntimeDirectRequestBodyMessage::Error(error))
            .await;
    }

    pub(super) async fn finish(self) -> io::Result<()> {
        self.sender
            .send(RuntimeDirectRequestBodyMessage::End)
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "gateway request body closed"))
    }
}

pub(super) fn runtime_direct_request_body_channel()
-> (RuntimeDirectRequestBodySender, RuntimeDirectRequestBody) {
    let (sender, receiver) = mpsc::channel(DIRECT_BODY_CHANNEL_CAPACITY.get());
    (
        RuntimeDirectRequestBodySender { sender },
        RuntimeDirectRequestBody {
            receiver,
            current: Bytes::new(),
            offset: 0,
            ended: false,
        },
    )
}

fn tiny_response_headers(response: &tiny_http::ResponseBox) -> Vec<(String, Vec<u8>)> {
    response
        .headers()
        .iter()
        .map(|header| {
            (
                header.field.as_str().as_str().to_string(),
                header.value.as_str().as_bytes().to_vec(),
            )
        })
        .collect()
}

fn append_header_if_missing(headers: &mut Vec<(String, Vec<u8>)>, name: &str, value: &[u8]) {
    if !headers
        .iter()
        .any(|(existing, _)| existing.eq_ignore_ascii_case(name))
    {
        headers.push((name.to_string(), value.to_vec()));
    }
}

fn copy_response_body(
    reader: &mut (dyn Read + Send),
    sender: GatewayResponseBodySender,
) -> io::Result<()> {
    let mut buffer = [0_u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return sender.finish(),
            Ok(read) => sender.blocking_send(Bytes::copy_from_slice(&buffer[..read]))?,
            Err(error) => {
                let kind = error.kind();
                let message = error.to_string();
                sender.blocking_send_error(error)?;
                return Err(io::Error::new(kind, message));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_request_body_distinguishes_clean_end_from_sender_drop() {
        let (sender, mut clean) = runtime_direct_request_body_channel();
        sender
            .sender
            .try_send(RuntimeDirectRequestBodyMessage::End)
            .unwrap();
        drop(sender);
        let mut byte = [0_u8; 1];
        assert_eq!(clean.read(&mut byte).unwrap(), 0);

        let (sender, mut truncated) = runtime_direct_request_body_channel();
        drop(sender);
        let error = truncated.read(&mut byte).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn direct_upgrade_has_single_handshake_headers_and_retains_permit() {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = Arc::new(Arc::clone(&semaphore).try_acquire_owned().unwrap());
        let (head, receiver) = oneshot::channel();
        let reply = RuntimeDirectReply {
            head: Some(head),
            permit,
        };
        let response = tiny_http::Response::new_empty(tiny_http::StatusCode(101))
            .with_header(tiny_http::Header::from_bytes("Connection", "Upgrade").unwrap())
            .with_header(tiny_http::Header::from_bytes("Upgrade", "websocket").unwrap())
            .boxed();

        let application = reply.upgrade("websocket", response).unwrap();
        let handled = receiver.blocking_recv().unwrap().unwrap();
        let headers = handled.response.headers();
        assert_eq!(headers.get_all("connection").iter().count(), 1);
        assert_eq!(headers.get_all("upgrade").iter().count(), 1);
        assert!(headers.get("content-length").is_none());
        assert!(semaphore.try_acquire().is_err());

        drop(application);
        drop(handled);
        assert!(semaphore.try_acquire().is_ok());
    }
}
