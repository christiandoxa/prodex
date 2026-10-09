
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
