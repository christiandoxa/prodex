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
    sender: mpsc::Sender<ChannelBodyMessage>,
}

enum ChannelBodyMessage {
    Data(Bytes),
    End,
    Error(GatewayBoxError),
}

impl GatewayResponseBodySender {
    pub fn blocking_send(&self, bytes: Bytes) -> io::Result<()> {
        self.sender
            .blocking_send(ChannelBodyMessage::Data(bytes))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "gateway response closed"))
    }

    pub fn blocking_send_error(self, error: io::Error) -> io::Result<()> {
        self.sender
            .blocking_send(ChannelBodyMessage::Error(Box::new(error)))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "gateway response closed"))
    }

    pub fn finish(self) -> io::Result<()> {
        self.sender
            .blocking_send(ChannelBodyMessage::End)
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "gateway response closed"))
    }
}

pub fn bounded_response_body(
    capacity: NonZeroUsize,
) -> (GatewayResponseBodySender, GatewayResponseBody) {
    bounded_response_body_with_guard(capacity, ())
}

pub fn bounded_response_body_with_guard(
    capacity: NonZeroUsize,
    guard: impl Send + 'static,
) -> (GatewayResponseBodySender, GatewayResponseBody) {
    let (sender, receiver) = mpsc::channel(capacity.get());
    (
        GatewayResponseBodySender { sender },
        ChannelBody {
            receiver,
            ended: false,
            _guard: Box::new(guard),
        }
        .boxed_unsync(),
    )
}

struct ChannelBody {
    receiver: mpsc::Receiver<ChannelBodyMessage>,
    ended: bool,
    _guard: Box<dyn Send>,
}

impl Body for ChannelBody {
    type Data = Bytes;
    type Error = GatewayBoxError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        if self.ended {
            return Poll::Ready(None);
        }
        match Pin::new(&mut self.receiver).poll_recv(context) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(ChannelBodyMessage::Data(bytes))) => {
                Poll::Ready(Some(Ok(Frame::data(bytes))))
            }
            Poll::Ready(Some(ChannelBodyMessage::End)) => {
                self.ended = true;
                Poll::Ready(None)
            }
            Poll::Ready(Some(ChannelBodyMessage::Error(error))) => {
                self.ended = true;
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                self.ended = true;
                Poll::Ready(Some(Err(Box::new(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "gateway response producer ended without an end marker",
                )))))
            }
        }
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

    #[tokio::test]
    async fn producer_drop_after_head_is_a_transport_error() {
        let (sender, mut body) = bounded_response_body(NonZeroUsize::MIN);
        let producer = thread::spawn(move || {
            sender
                .blocking_send(Bytes::from_static(b"partial"))
                .unwrap();
        });

        assert_eq!(
            body.frame().await.unwrap().unwrap().into_data().unwrap(),
            "partial"
        );
        let error = body.frame().await.unwrap().unwrap_err();
        assert!(error.to_string().contains("without an end marker"));
        assert!(body.frame().await.is_none());
        producer.join().unwrap();
    }
}
