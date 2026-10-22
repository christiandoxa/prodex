# Transcript message projection in Mojo (2026-10-22)

## Production ownership

Response messages and user/assistant/reasoning events now use operations 13–16
of the existing versioned transcript ABI. Mojo selects source roles/defaults,
message fields, eligible content parts and their LF join order. The Rust
role/content builders, event field mapping and message visibility adapter are
removed. There is no new parser, dependency or Rust fallback.

The compiled prompt JSON owner supplies shared payload visibility and
presence-before-type content selection. Its existing operation 3 and exact
prompt matching keep their contracts. Rust acquires the borrowed Serde tree
without wire serialization and invokes the existing compiled visible-message
normalizer. Redaction, final safe-text limits and event materialization retain
their existing owners. Normalizer absence maps to empty prepared message text;
that normalizer never returns a present empty string.

## Compatibility

A present non-string `text` blocks `content` fallback. Rendering skips invalid
or invisible parts while exact delivery matching still requires every part to
be valid and compares raw whitespace. Content arrays retain order and LF
separators. Missing/non-string roles default to `message`; string roles,
including empty strings, are preserved. Only response role `user` receives
metadata filtering. User/assistant event schemas use `message`; reasoning uses
`text`. Attachment-line filtering, leading spaces, bare internal CR, Unicode,
secret redaction and the final 65,536-byte text bound are covered at the caller.

Followed lines and the latest tail are bounded to 2 MiB, and Super verification
reads at most 512 KiB. These acquisition limits remain below the existing
64 MiB decoded-tree boundary. Unknown modes, invalid UTF-8 views, wrong view
counts and insufficient output capacity reject without changing output.

## Verification

Strict Cargo checks use `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`. Default CI/changed-test selection omits both.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/log_transcript_message_test.mojo`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/session_prompt_write_json_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::log_transcript_context::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::session_prompt_write::`
- `cargo test --locked -q -p prodex-app --lib app_commands::log::message_tests:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib app_commands::log:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib super_expose:: -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/super-wildcard-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `npm run ci -- --jobs 1`
- `npm run test:changed`
- `npm run docs`
- `node scripts/ci/churn-hygiene.mjs --staged --check`
- `git diff --cached --check`
- `git diff --check`

The listed checks pass: both direct Mojo suites, 14 transcript bridge tests,
three shared prompt bridge tests, five new caller tests, all 93 log tests,
all 120 Super expose tests, workspace Clippy, formatting and guards.
Default full CI passes in 504.62 seconds; changed tests pass in 3.61 seconds.
Default feature-off zero-test harnesses do not replace the strict positive
bridge/caller counts. Documentation and final staged checks pass.
Canonical share is 36.3730118227439% (136,844 Mojo / 239,380 Rust production
LOC). Counting rules, baselines and thresholds are unchanged. The 75% goal
remains open. Native non-Linux, live providers and exhaustive workspace tests
remain outside local coverage.
