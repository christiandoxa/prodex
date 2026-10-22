# Structured runtime-log message composition in Mojo (2026-10-22)

## Production ownership

`RuntimeProxyLogEvent::render_message` now calls the compiled message field
policy and renderer. Rust's empty-key predicate, separator decisions, key/value
assembly and unchecked capacity sum are deleted without a fallback. The host
only applies the checked selection flag and acquires sanitized keys, redacted
event/value text and Serde-quoted values. The original key still controls
sensitive-key redaction; its sanitized form controls display and field policy.

The new message-specific field ABI reuses the existing compiled field policy
and adds empty-key suppression. The older numeric facade retains its exact
flags, including `skip = false` for an empty key. Skipped values still avoid
redaction/quoting work. The ordered rendering input preserves duplicate fields;
the separate raw-key field map retains its last-key-wins behavior.

`prodex_mojo_structured_log_message_v1` uses caller-owned UTF-8 views, at most
65,536 field pairs and a 64 MiB aggregate text/output budget. It validates
version, counts, text, alignment and phase before writes. Measurement and short
capacity errors leave output canaries intact. Output allocation/status/UTF-8
validation reuses the existing private measured-text adapter, moved intact to
the crate root for both log and observability callers. No dependency, source
accounting rule, fallback or duplicate production algorithm is added.

## Verification

Strict Cargo checks set `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`. Default CI and changed tests omit those variables
and any archive override. Direct Mojo assertions use `-D ASSERT=all`.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/structured_log_message_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib log::structured`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime,mojo-rich,mojo-routing --lib log::structured`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib text_output::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-observability --lib observability::`
- `cargo test --locked -q -p prodex-runtime-proxy --lib structured_log_render -- --test-threads=1`
- `cargo test --locked -q -p prodex-runtime-proxy -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib 'main_internal_tests::runtime_proxy_' -- --test-threads=1`
- `node scripts/ci/test-serial.mjs --suite runtime-smoke`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/runtime-hotpath-guard.mjs --self-test`
- `node scripts/ci/runtime-hotpath-guard.mjs`
- `node scripts/ci/runtime-test-manifest-guard.mjs`
- `node scripts/compat/check-upstream-baseline.mjs`
- `node scripts/compat/capture-replay-fixture-tests.mjs`
- `npm run ci -- --jobs 1`
- `npm run test:changed`
- `npm run docs`
- `git diff --cached --check`
- `git diff --check`

The three caller regressions pass before and after migration with unchanged
expectations. Direct Mojo, five bridge tests in both complete feature variants,
the shared allocation check, 19 standalone observability, all 430 proxy-crate,
459 application proxy and 19 serialized smoke tests pass. Strict workspace
Clippy, formatting, ownership, source-share, no-fallback, size, hot-path,
runtime manifest, offline baseline/replay, secrets, wildcard and staged churn
checks pass. Full default CI passes in 720.89 seconds with 1,443 Rust tests;
changed tests pass as well. The native Windows installer check is intentionally
skipped on Linux; feature-off zero-test harnesses are not used as ABI evidence.

Evidence retains the initial missing test-only `BTreeMap` import. A partial
`mojo-rich,mojo-routing` test configuration failed to link runtime-owned
session/header symbols; the complete `mojo-runtime,mojo-rich,mojo-routing`
configuration passes without changing feature policy or adding a fallback.
The official share is 36.40482782235695% (137,149 Mojo / 239,584 Rust
production LOC). The 75% target remains open. Native non-Linux execution,
live providers, and exhaustive workspace/all-serialized suites are outside
local coverage.
