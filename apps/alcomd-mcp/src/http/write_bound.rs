//! A write stall is bounded independently of an idle subscription body.
use std::future::Future;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::Sleep;

pub(super) struct WriteBound<S> {
    io: S,
    budget: Duration,
    blocked: Option<Pin<Box<Sleep>>>,
}
impl<S> WriteBound<S> {
    pub fn new(io: S, budget: Duration) -> Self {
        Self {
            io,
            budget,
            blocked: None,
        }
    }
    fn pending<T>(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<T>> {
        let timer = self
            .blocked
            .get_or_insert_with(|| Box::pin(tokio::time::sleep(self.budget)));
        if tokio::time::Instant::now() >= timer.deadline() || timer.as_mut().poll(cx).is_ready() {
            Poll::Ready(Err(io::ErrorKind::TimedOut.into()))
        } else {
            Poll::Pending
        }
    }
}
impl<S: AsyncRead + Unpin> AsyncRead for WriteBound<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_read(cx, buffer)
    }
}
impl<S: AsyncWrite + Unpin> AsyncWrite for WriteBound<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        match Pin::new(&mut self.io).poll_write(cx, buffer) {
            Poll::Ready(result) => {
                self.blocked = None;
                Poll::Ready(result)
            }
            Poll::Pending => self.pending(cx),
        }
    }
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffers: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        match Pin::new(&mut self.io).poll_write_vectored(cx, buffers) {
            Poll::Ready(result) => {
                self.blocked = None;
                Poll::Ready(result)
            }
            Poll::Pending => self.pending(cx),
        }
    }
    fn is_write_vectored(&self) -> bool {
        self.io.is_write_vectored()
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match Pin::new(&mut self.io).poll_flush(cx) {
            Poll::Ready(result) => {
                self.blocked = None;
                Poll::Ready(result)
            }
            Poll::Pending => self.pending(cx),
        }
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match Pin::new(&mut self.io).poll_shutdown(cx) {
            Poll::Ready(result) => {
                self.blocked = None;
                Poll::Ready(result)
            }
            Poll::Pending => self.pending(cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    #[tokio::test]
    async fn stalled_output_terminates_without_an_unbounded_queue() {
        let (stream, _unread_peer) = tokio::io::duplex(1);
        let mut stream = WriteBound::new(stream, Duration::from_millis(20));
        let result = tokio::time::timeout(Duration::from_secs(1), stream.write_all(b"ab"))
            .await
            .expect("bounded output");
        assert_eq!(
            result.expect_err("no consumer").kind(),
            io::ErrorKind::TimedOut
        );
    }
    #[tokio::test]
    async fn waiting_for_a_notification_does_not_start_the_write_timer() {
        let (stream, _peer) = tokio::io::duplex(1);
        let mut stream = WriteBound::new(stream, Duration::from_millis(10));
        tokio::time::sleep(Duration::from_millis(30)).await;
        stream
            .write_all(b"a")
            .await
            .expect("idle is not blocked output");
    }
}
