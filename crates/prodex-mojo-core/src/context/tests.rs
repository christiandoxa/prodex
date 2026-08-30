use super::{
    CONTEXT_TEXT_ABI_VERSION, ContextSignalLine, ContextTextRowsResult, ProdexStringView,
    gemini_glob_matches, prepare_signal_rows, prodex_context_prepare_signal_rows_v1,
    text_abi_layout_matches, text_abi_version,
};

#[test]
fn text_abi_accepts_utf8_embedded_nul_empty_and_unsentinelled_views() {
    assert_eq!(text_abi_version(), Ok(CONTEXT_TEXT_ABI_VERSION));
    assert!(text_abi_layout_matches());

    let unicode = "账户🙂e\u{301}\0東京";
    assert_eq!(raw_status(unicode.as_ptr(), unicode.len(), 8).0, 0);
    assert_eq!(raw_status(std::ptr::null(), 0, 8).0, 0);

    let no_sentinel = [b'e', b'r', b'r', b'o', 0xff];
    assert_eq!(raw_status(no_sentinel.as_ptr(), 4, 8).0, 0);
}

#[test]
fn text_abi_rejects_malformed_utf8_null_nonempty_and_short_output() {
    for malformed in [
        &[0x80][..],
        &[0xc0, 0xaf],
        &[0xe0, 0x80, 0x80],
        &[0xed, 0xa0, 0x80],
        &[0xf0, 0x90, 0x80],
        &[0xf4, 0x90, 0x80, 0x80],
        &[0xff],
    ] {
        assert_eq!(raw_status(malformed.as_ptr(), malformed.len(), 8).0, 2);
    }
    assert_eq!(raw_status("🔥".as_ptr(), 3, 8).0, 2);
    assert_eq!(raw_status(std::ptr::null(), 1, 8).0, 2);
    assert_eq!(raw_status_version(0, b"error".as_ptr(), 5, 8).0, 4);

    let (status, result) = raw_status(b"error".as_ptr(), 5, 7);
    assert_eq!(status, 1);
    assert_eq!(result.required_before_rows, 8);
    assert_eq!(result.required_key_capacity, 1);
    assert_eq!(result.required_hash_capacity, 2);
}

#[test]
fn text_pipeline_is_reentrant_across_concurrent_calls() {
    let threads = (0..8)
        .map(|_| {
            std::thread::spawn(|| {
                let signal = ContextSignalLine {
                    text: "error: 并行🙂\0",
                    counts: [1, 0, 0, 0, 0, 0, 0],
                };
                for _ in 0..100 {
                    let rows = prepare_signal_rows(&[signal, signal], &[signal]).unwrap();
                    assert_eq!(rows.after_available, [1]);
                    assert_eq!(
                        rows.before_rows
                            .as_chunks::<8>()
                            .0
                            .iter()
                            .map(|row| row[0])
                            .collect::<Vec<_>>(),
                        [0, 0]
                    );
                }
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }
}

#[test]
fn gemini_glob_matches_request_context_patterns() {
    for (pattern, path, expected) in [
        ("**/*.rs", "src/lib.rs", true),
        ("**/*.rs", "lib.rs", true),
        ("src/*.RS", "src/lib.rs", true),
        ("src/?ib.rs", "src/lib.rs", true),
        ("src/*.rs", "src/nested/lib.rs", false),
        ("a/**/b", "a/b", true),
        ("a/**/b", "a/x/y/b", true),
        ("a/**/b", "a/x/y/c", false),
        ("a/**/b", "a/b/c", false),
        ("a/**", "a", true),
        ("a/**", "a/x/y", true),
        ("a/*/b", "a//b", true),
        ("a/*/b", "a/x/y/b", false),
        ("ab*cd", "abXYZcd", true),
        ("*a*b", "xxaYYb", true),
        ("a/", "a/", true),
        ("a/", "a", false),
    ] {
        assert_eq!(gemini_glob_matches(pattern, path), Ok(expected));
    }
}

fn raw_status(
    ptr: *const u8,
    len: usize,
    before_rows_capacity: usize,
) -> (i64, ContextTextRowsResult) {
    raw_status_version(CONTEXT_TEXT_ABI_VERSION, ptr, len, before_rows_capacity)
}

fn raw_status_version(
    abi_version: i64,
    ptr: *const u8,
    len: usize,
    before_rows_capacity: usize,
) -> (i64, ContextTextRowsResult) {
    let before_views = [ProdexStringView { ptr, len }];
    let before_counts = [1_i64, 0, 0, 0, 0, 0, 0];
    let after_views: [ProdexStringView; 0] = [];
    let after_counts: [i64; 0] = [];
    let mut before_rows = vec![0_i64; before_rows_capacity.max(1)];
    let mut after_available = [0_i64; 1];
    let mut hash_slots = [-1_i64; 2];
    let mut key_hashes = [0_u64; 1];
    let mut key_sources = [0_i64; 1];
    let mut key_indices = [0_i64; 1];
    let mut result = ContextTextRowsResult::default();
    let status = unsafe {
        prodex_context_prepare_signal_rows_v1(
            abi_version,
            before_views.as_ptr(),
            before_counts.as_ptr(),
            1,
            after_views.as_ptr(),
            after_counts.as_ptr(),
            0,
            before_rows.as_mut_ptr(),
            before_rows_capacity as i64,
            after_available.as_mut_ptr(),
            1,
            hash_slots.as_mut_ptr(),
            2,
            key_hashes.as_mut_ptr(),
            key_sources.as_mut_ptr(),
            key_indices.as_mut_ptr(),
            &mut result,
        )
    };
    (status, result)
}
