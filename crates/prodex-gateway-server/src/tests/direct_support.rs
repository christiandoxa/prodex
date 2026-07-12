use std::{
    convert::Infallible,
    future::Future,
    num::NonZeroUsize,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};

use bytes::Bytes;
use http_body_util::Full;
use hyper::{
    Response, StatusCode,
    body::{Body, Frame},
};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    time::timeout,
};

use crate::{
    GatewayHandlerRequest, GatewayHandlerResponse, GatewayHandlerResult, GatewayServerConfig,
    GatewayServerMode, bounded_in_process_upgrade, run_with_handler,
};

use super::TestFrontend;

pub(super) struct GatedBody {
    state: u8,
    release: oneshot::Receiver<()>,
    dropped: Option<oneshot::Sender<()>>,
}

impl GatedBody {
    pub(super) fn new(
        release: oneshot::Receiver<()>,
        dropped: Option<oneshot::Sender<()>>,
    ) -> Self {
        Self {
            state: 0,
            release,
            dropped,
        }
    }
}

impl Body for GatedBody {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        match this.state {
            0 => {
                this.state = 1;
                Poll::Ready(Some(Ok(Frame::data(Bytes::from_static(b"first")))))
            }
            1 => match Pin::new(&mut this.release).poll(context) {
                Poll::Ready(_) => {
                    this.state = 2;
                    Poll::Ready(Some(Ok(Frame::data(Bytes::from_static(b"second")))))
                }
                Poll::Pending => Poll::Pending,
            },
            _ => Poll::Ready(None),
        }
    }

    fn is_end_stream(&self) -> bool {
        self.state >= 2
    }
}

impl Drop for GatedBody {
    fn drop(&mut self) {
        if let Some(dropped) = self.dropped.take() {
            let _ = dropped.send(());
        }
    }
}

pub(super) struct DropSignal(Option<oneshot::Sender<()>>);

impl DropSignal {
    pub(super) fn new(dropped: oneshot::Sender<()>) -> Self {
        Self(Some(dropped))
    }
}

impl Drop for DropSignal {
    fn drop(&mut self) {
        if let Some(dropped) = self.0.take() {
            let _ = dropped.send(());
        }
    }
}

pub(super) fn full_response(body: &'static [u8]) -> GatewayHandlerResult {
    Ok(GatewayHandlerResponse::new(Response::new(Full::new(
        Bytes::from_static(body),
    ))))
}

pub(super) async fn spawn_direct_frontend<H, Fut>(
    mode: GatewayServerMode,
    configure: impl FnOnce(&mut GatewayServerConfig),
    handler: H,
) -> TestFrontend
where
    H: Fn(GatewayHandlerRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = GatewayHandlerResult> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let mut config = GatewayServerConfig::production(addr, mode);
    config.response_header_timeout = Duration::from_secs(1);
    config.drain_timeout = Duration::from_secs(1);
    configure(&mut config);
    let (stop_tx, stop_rx) = oneshot::channel();
    let task = tokio::spawn(run_with_handler(listener, config, handler, async move {
        stop_rx.await.map_err(anyhow::Error::from)
    }));
    (addr, stop_tx, task)
}

#[tokio::test]
async fn in_process_upgrade_preserves_byte_order_and_closes_before_drain() {
    let (closed_tx, closed_rx) = oneshot::channel();
    let closed_tx = Arc::new(Mutex::new(Some(closed_tx)));
    let handler_closed = Arc::clone(&closed_tx);
    let (front_addr, stop, mut front) = spawn_direct_frontend(
        GatewayServerMode::DataPlane,
        |_| {},
        move |_| {
            let closed = Arc::clone(&handler_closed);
            async move {
                let (mut application, handoff) = bounded_in_process_upgrade(NonZeroUsize::MIN);
                std::thread::spawn(move || {
                    let mut payload = [0_u8; 8];
                    std::io::Read::read_exact(&mut application, &mut payload).unwrap();
                    assert_eq!(&payload, b"pingpong");
                    std::io::Write::write_all(&mut application, b"firstsecond").unwrap();
                    let mut byte = [0_u8; 1];
                    assert_eq!(std::io::Read::read(&mut application, &mut byte).unwrap(), 0);
                    closed.lock().unwrap().take().unwrap().send(()).unwrap();
                });
                let response = Response::builder()
                    .status(StatusCode::SWITCHING_PROTOCOLS)
                    .header(hyper::header::CONNECTION, "Upgrade")
                    .header(hyper::header::UPGRADE, "websocket")
                    .body(Full::new(Bytes::new()))
                    .unwrap();
                Ok(GatewayHandlerResponse::with_in_process_upgrade(
                    response, handoff,
                ))
            }
        },
    )
    .await;
    let mut client = TcpStream::connect(front_addr).await.unwrap();
    client
        .write_all(
            b"GET /realtime HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n",
        )
        .await
        .unwrap();
    assert!(
        super::read_headers(&mut client)
            .await
            .starts_with("HTTP/1.1 101")
    );
    client.write_all(b"ping").await.unwrap();
    client.write_all(b"pong").await.unwrap();
    let mut payload = [0_u8; 11];
    client.read_exact(&mut payload).await.unwrap();
    assert_eq!(&payload, b"firstsecond");

    stop.send(()).unwrap();
    timeout(Duration::from_secs(1), &mut front)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let mut byte = [0_u8; 1];
    assert_eq!(client.read(&mut byte).await.unwrap(), 0);
    timeout(Duration::from_secs(1), closed_rx)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn handler_overload_is_a_fail_fast_service_unavailable() {
    let (front_addr, stop, front) = spawn_direct_frontend(
        GatewayServerMode::DataPlane,
        |_| {},
        |_| async { Err(crate::GatewayHandlerError::Overloaded) },
    )
    .await;
    let started = tokio::time::Instant::now();
    let mut client = TcpStream::connect(front_addr).await.unwrap();
    client
        .write_all(b"POST /responses HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n")
        .await
        .unwrap();

    let headers = super::read_headers(&mut client).await;
    assert!(headers.starts_with("HTTP/1.1 503"));
    assert!(started.elapsed() < Duration::from_millis(100));

    stop.send(()).unwrap();
    front.await.unwrap().unwrap();
}
