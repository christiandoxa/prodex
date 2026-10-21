fn read_mock_request(stream: &mut impl Read) {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut request = Vec::new();
    let mut chunk = [0_u8; 256];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        assert!(Instant::now() < deadline, "mock request headers timed out");
        match stream.read(&mut chunk) {
            Ok(0) => panic!("mock request ended before complete headers"),
            Ok(read) => request.extend_from_slice(&chunk[..read]),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut
                        | io::ErrorKind::WouldBlock
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(error) => panic!("mock request headers could not be read: {error}"),
        }
    }
}

#[test]
fn mock_request_reader_waits_through_header_read_timeouts() {
    struct DelayedHeaders {
        timeout_pending: bool,
        body: std::io::Cursor<&'static [u8]>,
    }

    impl Read for DelayedHeaders {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if std::mem::take(&mut self.timeout_pending) {
                return Err(io::ErrorKind::TimedOut.into());
            }
            self.body.read(buffer)
        }
    }

    let headers = b"GET /v1/responses HTTP/1.1\r\nHost: example.com\r\n\r\n";
    let mut reader = DelayedHeaders {
        timeout_pending: true,
        body: std::io::Cursor::new(headers.as_slice()),
    };
    read_mock_request(&mut reader);
    assert_eq!(reader.body.position(), headers.len() as u64);
}

#[test]
fn async_prefetch_splits_large_upstream_chunks_before_queueing() {
    let expected = vec![b'x'; super::RUNTIME_LOCAL_REWRITE_STREAM_CHUNK_BYTES * 3 + 7];
    let (mut response, runtime, server, _) = mock_async_sse_response(Vec::new(), 100, false);
    response.pending = expected.clone();
    let mut prefetch = super::RuntimeLocalRewriteSsePrefetch::spawn(response, None);
    let mut actual = Vec::new();
    loop {
        match prefetch
            .recv_timeout(Duration::from_secs(1))
            .expect("bounded upstream chunk should arrive")
        {
            super::RuntimeLocalRewritePrefetchChunk::Data(chunk) => {
                assert!(chunk.len() <= super::RUNTIME_LOCAL_REWRITE_STREAM_CHUNK_BYTES);
                actual.extend_from_slice(&chunk);
            }
            super::RuntimeLocalRewritePrefetchChunk::End => break,
            super::RuntimeLocalRewritePrefetchChunk::Error(_, message) => {
                panic!("unexpected bounded stream error: {message}")
            }
        }
    }
    assert_eq!(actual, expected);
    runtime.block_on(async { tokio::task::yield_now().await });
    server.join().expect("mock upstream should finish");
}

#[test]
fn async_prefetch_drop_aborts_pump_and_releases_permit() {
    let (response, runtime, server, closed) =
        mock_async_sse_response(vec![(Duration::ZERO, b"first".to_vec())], 100, true);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
    let permit = Arc::clone(&semaphore)
        .try_acquire_owned()
        .expect("prefetch slot should be available");
    let prefetch = super::RuntimeLocalRewriteSsePrefetch::spawn(response, Some(permit));
    let cancelled = Arc::clone(&prefetch.cancelled);
    assert_eq!(semaphore.available_permits(), 0);
    drop(prefetch);
    assert!(cancelled.load(Ordering::Acquire));
    assert_eq!(semaphore.available_permits(), 1);
    runtime.block_on(async { tokio::time::sleep(Duration::from_millis(25)).await });
    server
        .join()
        .expect("mock upstream should observe cancellation");
    assert!(closed.load(Ordering::Acquire));
}
