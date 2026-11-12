//! Strict bounded framing around the SDK's typed messages and response writer.
//! Unlike its general-purpose reader, malformed input receives a protocol error.

use std::{future::Future, io, pin::Pin};

use rmcp::{
    ErrorData, RoleServer,
    service::{RxJsonRpcMessage, TxJsonRpcMessage},
    transport::{Transport, async_rw::AsyncRwTransport},
};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, BufReader, Empty};

type PendingReply = Pin<Box<dyn Future<Output = io::Result<()>> + Send>>;

pub(crate) struct StdioTransport<R: AsyncRead, W: AsyncWrite> {
    input: BufReader<R>,
    writer: AsyncRwTransport<RoleServer, Empty, W>,
    frame: Vec<u8>,
    discarding: bool,
    pending_reply: Option<PendingReply>,
}

enum Frame {
    Message(Vec<u8>),
    Oversized,
    Incomplete,
}

impl<R, W> StdioTransport<R, W>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send + 'static,
{
    pub(crate) fn new(input: R, output: W) -> Self {
        Self {
            input: BufReader::new(input),
            writer: AsyncRwTransport::new(tokio::io::empty(), output),
            frame: Vec::new(),
            discarding: false,
            pending_reply: None,
        }
    }

    // All consumed bytes, including discard state, live on self. SDK response
    // dispatch can cancel this future between reads without losing a frame.
    async fn read_frame(&mut self) -> io::Result<Option<Frame>> {
        loop {
            let available = self.input.fill_buf().await?;
            if available.is_empty() {
                if std::mem::take(&mut self.discarding) {
                    return Ok(Some(Frame::Oversized));
                }
                if !self.frame.is_empty() {
                    self.frame.clear();
                    return Ok(Some(Frame::Incomplete));
                }
                return Ok(None);
            }
            let newline = available.iter().position(|byte| *byte == b'\n');
            let length = newline.unwrap_or(available.len());
            let consumed = length + usize::from(newline.is_some());
            if !self.discarding {
                if self.frame.len() + length > alcomd_protocol::MAX_FRAME_PAYLOAD_BYTES {
                    self.frame.clear();
                    self.discarding = true;
                } else {
                    self.frame.extend_from_slice(&available[..length]);
                }
            }
            self.input.consume(consumed);
            if newline.is_some() {
                return Ok(Some(if std::mem::take(&mut self.discarding) {
                    Frame::Oversized
                } else {
                    Frame::Message(std::mem::take(&mut self.frame))
                }));
            }
        }
    }
}

impl<R, W> Transport<RoleServer> for StdioTransport<R, W>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send + 'static,
{
    type Error = io::Error;

    fn send(
        &mut self,
        message: TxJsonRpcMessage<RoleServer>,
    ) -> impl Future<Output = io::Result<()>> + Send + 'static {
        self.writer.send(message)
    }

    async fn receive(&mut self) -> Option<RxJsonRpcMessage<RoleServer>> {
        loop {
            if let Some(reply) = self.pending_reply.as_mut() {
                if reply.await.is_err() {
                    return None;
                }
                self.pending_reply = None;
            }
            let error = match self.read_frame().await {
                Ok(Some(Frame::Message(bytes))) => {
                    match serde_json::from_slice::<RxJsonRpcMessage<RoleServer>>(&bytes) {
                        Ok(message) => return Some(message),
                        Err(error) if error.is_syntax() || error.is_eof() => {
                            ErrorData::parse_error("Invalid JSON message", None)
                        }
                        Err(_) => ErrorData::invalid_request("Invalid request", None),
                    }
                }
                Ok(Some(Frame::Oversized)) => {
                    ErrorData::invalid_request("Message exceeds the frame size limit", None)
                }
                Ok(Some(Frame::Incomplete)) => {
                    ErrorData::parse_error("Incomplete NDJSON message", None)
                }
                Ok(None) => return None,
                Err(error) => {
                    // OS error categories suffice; never log an incoming payload.
                    eprintln!("MCP input failed: {:?}", error.kind());
                    return None;
                }
            };
            // Keep the write future across cancellation of receive: a partially
            // written error must neither be truncated nor sent a second time.
            self.pending_reply = Some(Box::pin(
                self.writer
                    .send(TxJsonRpcMessage::<RoleServer>::error(error, None)),
            ));
        }
    }

    async fn close(&mut self) -> io::Result<()> {
        self.writer.close().await
    }
}
