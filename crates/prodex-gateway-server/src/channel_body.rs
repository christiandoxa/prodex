use std::{
    io,
    num::NonZeroUsize,
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Bytes;
use http_body_util::BodyExt as _;
use hyper::body::{Body, Frame};
use tokio::sync::mpsc;

use crate::{GatewayBoxError, GatewayResponseBody};

pub struct GatewayResponseBodySender {
    sender: mpsc::Sender<Result<Bytes, GatewayBoxError>>,
}

impl GatewayResponseBodySender {
    pub fn blocking_send(&self, bytes: Bytes) -> io::Result<()> {
        self.sender
            .blocking_send(Ok(bytes))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "gateway response closed"))
    }

    pub fn blocking_send_error(self, error: io::Error) -> io::Result<()> {
        self.sender
            .blocking_send(Err(Box::new(error)))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "gateway response closed"))
    }
}

pub fn bounded_response_body(
    capacity: NonZeroUsize,
) -> (GatewayResponseBodySender, GatewayResponseBody) {
    let (sender, receiver) = mpsc::channel(capacity.get());
    (
        GatewayResponseBodySender { sender },
        ChannelBody { receiver }.boxed_unsync(),
    )
}

struct ChannelBody {
    receiver: mpsc::Receiver<Result<Bytes, GatewayBoxError>>,
}

impl Body for ChannelBody {
    type Data = Bytes;
    type Error = GatewayBoxError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.receiver)
            .poll_recv(context)
            .map(|item| item.map(|item| item.map(Frame::data)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc as std_mpsc;
    use std::thread;
    use std::time::Duration;

    #[tokio::test]
    async fn bounded_body_preserves_order_backpressure_error_and_cancellation() {
        let (sender, mut body) = bounded_response_body(NonZeroUsize::MIN);
        let (first_sent, first_seen) = std_mpsc::channel();
        let (second_sent, second_seen) = std_mpsc::channel();
        let producer = thread::spawn(move || {
            sender.blocking_send(Bytes::from_static(b"first")).unwrap();
            first_sent.send(()).unwrap();
            sender.blocking_send(Bytes::from_static(b"second")).unwrap();
            second_sent.send(()).unwrap();
            sender
                .blocking_send_error(io::Error::other("stream failed"))
                .unwrap();
        });
        first_seen.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(second_seen.recv_timeout(Duration::from_millis(20)).is_err());
        assert_eq!(
            body.frame().await.unwrap().unwrap().into_data().unwrap(),
            "first"
        );
        second_seen.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(
            body.frame().await.unwrap().unwrap().into_data().unwrap(),
            "second"
        );
        assert!(body.frame().await.unwrap().is_err());
        assert!(body.frame().await.is_none());
        producer.join().unwrap();

        let (sender, body) = bounded_response_body(NonZeroUsize::MIN);
        drop(body);
        let error = thread::spawn(move || sender.blocking_send(Bytes::from_static(b"cancelled")))
            .join()
            .unwrap()
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    }
}
