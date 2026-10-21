# Session JSONL decoded-line cursor and compressed-name policy in Mojo (2026-10-21)

## Production ownership

- **Consumer:** `crates/prodex-session-store/src/session_file.rs`, the
  actual reader shared by session report, recent workspace, resumption
  and compacted compressed JSONL session paths.
- **Compiled owner:** `mojo/prodex_core/session_selector.mojo` exports
  two independent versioned FFI kernels:
  `prodex_session_file_line_frame_v1` and
  `prodex_session_file_compressed_suffix_v1`. The checked bridge lives
  under `crates/prodex-mojo-core/src/json/session_file_scan.rs`.
- Mojo exclusively performs saturating decoded byte-cursor arithmetic,
  size-cap evaluation relative to the original scan start, empty/EOF
  classification, trailing LF detection and incomplete-line offset
  rewind. The host's old Rust `saturating_add`, `saturating_sub`,
  `line.ends_with`, `read == 0` branches were deleted.
- Mojo also exclusively decides whether a UTF-8 session file basename
  ends with the exact case-sensitive `.jsonl.zst` suffix, reusing the
  existing compiled session filename suffix comparison. The old Rust
  `str::ends_with` compressed-file classifier was deleted.
- Rust retains OS-aware path acquisition, non-UTF-8 filename handling,
  no-symlink/regular-file checks, BufReader, incremental decompression,
  UTF-8 `String` line acquisition, callback execution and report
  materialization. An ABI failure aborts; no Rust code recomputes the
  selected state.
- A decoded line is borrowed for only the ABI call; output is two
  strictly checked u64 words (action and complete offset). The Mojo
  kernel conserves the old edge behavior even at `u64::MAX` where
  an incomplete-line result uses the saturated next offset minus the
  actual read length, rather than the earlier pre-read offset.

## Offline regression and validation

- Direct Mojo fixture covers EOF, exact budget, oversize, incomplete
  and complete lines, exact compressed suffixes, invalid ABI and bad
  source pointers.
- Two Rust compiled bridge tests cover saturation extrema, scan start,
  exact `.jsonl.zst` recognition and negative suffix cases.
- Real file caller regressions cover binary/non-UTF-8 filenames,
  incomplete JSONL scan resume and incremental gzip/zstd processing.
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/session_file_line_frame_test.mojo`:
  PASS.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib compiled_scan_frame`:
  one passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib compiled_compressed_suffix`:
  one passed.
- `cargo test --locked -q -p prodex-session-store`: 63 passed.
- Strict `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`,
  Cargo format, Mojo ownership/no-fallback, size, source-share,
  Markdown and whitespace guards: PASS.
- Canonical audited production share 36.26493029414748%
  (135,839 Mojo / 238,735 Rust production LOC). Project 75% remains
  outstanding; neither count formula nor release version was changed.
