# Shared session image-tag scanning in compiled Mojo (2026-10-21)

## Production ownership

The existing shared-attachment owner now scans complete session text and emits
all image path byte ranges through `prodex_shared_image_tag_paths_v1`.
`codex_session_image_attachments_are_stable`,
`codex_session_persisted_attachment_paths` and
`rewrite_codex_session_image_paths` consume those ranges. Their three Rust
`<image ` / `>` search loops and per-tag adapter shim are deleted.

Mojo reuses the original `shared_image_tag_range` lexer. Observations advance to
the closing tag delimiter; rewriting advances to the selected path end. These
distinct cursor rules preserve nested-tag behavior, skipped malformed tags,
escaped-path precedence, empty paths and original UTF-8 byte ranges. Rust
retains platform path handling, no-follow file validation, copying, permissions,
compression, JSON escaping and durable writes.

## ABI and tests

The v1 ABI bounds the original UTF-8 view to the existing 64 MiB session limit.
It validates mode, addresses, alignment and capacities, counts ranges before
writing, and reports required row capacity. Insufficient capacity leaves row
storage unchanged. Rust allocates exactly that count and validates count,
monotonicity, every range and UTF-8 boundaries; it never recomputes the scan.

Direct Mojo and Rust bridge tests cover literal/escaped paths, Unicode, nested
cursor differences, malformed/unclosed tags, empty paths, empty input, 2,000
image tags, ABI/mode errors, null/misaligned pointers, oversized input,
malformed UTF-8 and unchanged rows on capacity rejection. The filesystem
regression verifies nested-path copying and exact surrounding text, persisted
references and stability. The no-fallback guard rejects restored Rust scanning
and requires all three production caller modes.

## Validation commands

Run direct compiled Cargo checks with the pinned Mojo 1.1.0 compiler and
`PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0`:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/shared_image_tag_batch_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib shared_attachment_policy::`
- `cargo test --locked -q -p prodex-shared-codex-fs`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `npm run docs`
- `npm run test:changed`
- `git diff --check`

The first direct Mojo test exposed an incorrect expected byte offset in its
second path fixture; correcting that expectation made the program pass. The
first workspace Clippy check rejected `anyhow::Context` on `MojoError`; the
caller now explicitly maps the bounded, path-free Mojo error. Validation is
repeated on the corrected implementation.

The 75% target remains open. Live provider traffic and native non-Linux execution
are outside this offline checkpoint.

All listed checks pass on the corrected implementation: seven strict compiled
bridge tests, 95 filesystem tests, direct Mojo assertions, strict workspace
Clippy, guard self-tests, documentation and the default changed-test runner.

After rebasing onto the terminal-rendering checkpoint, the strict seven-test
bridge suite, 95-test filesystem suite, workspace Clippy, all guards and
changed-test runner pass again. The crate-boundary guard and self-test pass.
Canonical production share is 36.26932905386101% (135,829 Mojo / 238,672 Rust
production LOC), with the 75% target still open.
