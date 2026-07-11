use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use bytes::Bytes;
use http_body_util::Full;
use hyper::{
    Response,
    body::{Body, Frame},
};
use tokio::{net::TcpListener, sync::oneshot};

use crate::{
    GatewayHandlerRequest, GatewayHandlerResponse, GatewayHandlerResult, GatewayServerConfig,
    GatewayServerMode, run_with_handler,
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
