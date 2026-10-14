# Runtime-doctor plain-text log envelope migration

The runtime-doctor log parser now calls a compiled, versioned Mojo
`prodex_mojo_log_plain_envelope_v1` entrypoint to locate the first `] `
delimiter, decide the message start, and select an optional bracketed
timestamp span. Rust's former `find`, `strip_prefix`, and `split_once`
envelope decisions were removed with no Rust fallback or recomputation.

The production consumer is
`crates/prodex-runtime-doctor/src/parsing/log_line.rs` through the
`prodex_mojo_core::rich::parse_plain_log_envelope` adapter. Rust still
owns UTF-8 slicing, Unicode whitespace normalization, JSON decoding
and JSON field precedence, and secret-safe redaction. The Mojo entrypoint
rejects bad ABI versions, null pointers with nonempty input, and undersized
result buffers. Its output is range-validated before slicing.

## Verification

- Strict `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0` Mojo-core
  direct ABI filter `plain_log_envelope`: 2 passed.
- Strict `prodex-runtime-doctor --features mojo --lib` caller suite:
  49 passed. The new test covers bracketed timestamps, Unicode,
  first-delimiter precedence, unterminated lines, and JSON priority.
- `node scripts/ci/mojo-ownership.mjs --check`: pass.
- `node scripts/ci/mojo-authority-guard.mjs`: pass.
- `node scripts/ci/mojo-no-fallback-guard.mjs --check`: pass.
- `node scripts/ci/mojo-production-share.mjs --check`: pass.
- `git diff --check`: pass.

The broad production share after this checkpoint is 32.94%
(108,862 Mojo LOC / 330,506 combined LOC); 75% remains outstanding.
No CI-green gating is implied by this local checkpoint.
