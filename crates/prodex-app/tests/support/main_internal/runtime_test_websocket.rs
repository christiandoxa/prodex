use super::*;

pub(super) struct TwoChunkReader {
    chunks: Vec<Vec<u8>>,
    index: usize,
    offset: usize,
}

impl TwoChunkReader {
    fn new(chunks: Vec<Vec<u8>>) -> Self {
        Self {
            chunks,
            index: 0,
            offset: 0,
        }
    }
}

impl Read for TwoChunkReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.index >= self.chunks.len() {
            return Ok(0);
        }
        let chunk = &self.chunks[self.index];
        let remaining = &chunk[self.offset..];
        let len = remaining.len().min(buf.len());
        buf[..len].copy_from_slice(&remaining[..len]);
        self.offset += len;
        if self.offset >= chunk.len() {
            self.index += 1;
            self.offset = 0;
        }
        Ok(len)
    }
}

pub(super) struct FailAfterFirstChunkWriter {
    saw_first_chunk_body: bool,
}

impl FailAfterFirstChunkWriter {
    fn new() -> Self {
        Self {
            saw_first_chunk_body: false,
        }
    }
}

impl Write for FailAfterFirstChunkWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.saw_first_chunk_body {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "synthetic local writer disconnect",
            ));
        }
        if buf.starts_with(b"data:") {
            self.saw_first_chunk_body = true;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.saw_first_chunk_body {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "synthetic local writer disconnect",
            ));
        }
        Ok(())
    }
}

pub(super) struct FailOnUnexpectedMidStreamFlushWriter {
    flush_count: usize,
    saw_trailer: bool,
}

impl FailOnUnexpectedMidStreamFlushWriter {
    fn new() -> Self {
        Self {
            flush_count: 0,
            saw_trailer: false,
        }
    }
}

impl Write for FailOnUnexpectedMidStreamFlushWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf == b"0\r\n\r\n" {
            self.saw_trailer = true;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.flush_count >= 2 && !self.saw_trailer {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "unexpected non-SSE chunk flush before trailer",
            ));
        }
        self.flush_count += 1;
        Ok(())
    }
}

pub(super) fn set_test_websocket_io_timeout(
    socket: &mut WsSocket<MaybeTlsStream<TcpStream>>,
    timeout: Duration,
) {
    match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => {
            let _ = stream.set_read_timeout(Some(timeout));
            let _ = stream.set_write_timeout(Some(timeout));
        }
        MaybeTlsStream::Rustls(stream) => {
            let _ = stream.sock.set_read_timeout(Some(timeout));
            let _ = stream.sock.set_write_timeout(Some(timeout));
        }
        _ => {}
    }
}

pub(super) fn runtime_test_local_websocket_pair()
-> (RuntimeLocalWebSocket, RuntimeUpstreamWebSocket) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("test websocket listener should bind");
    let addr = listener
        .local_addr()
        .expect("test websocket listener should expose an address");
    let client = thread::spawn(move || {
        let (socket, _response) =
            ws_connect(format!("ws://{addr}")).expect("test websocket client should connect");
        socket
    });
    let (server_stream, _addr) = listener
        .accept()
        .expect("test websocket server should accept a connection");
    let local_socket =
        tungstenite::accept(Box::new(server_stream) as Box<dyn TinyReadWrite + Send>)
            .expect("test websocket server handshake should succeed");
    let client_socket = client
        .join()
        .expect("test websocket client thread should join");
    (local_socket, client_socket)
}
