# Shared Presidio JSON selection — 2026-10-19

`redaction_json.mojo` owns the bounded schema-aware traversal used by
Presidio collection and masking. Its versioned production ABI selects
preorder node indices, sanitized display paths, inherited sensitive kinds,
coverage and depth/value/text-budget failures. It reuses the existing
redaction field classifier, with no Rust traversal-policy fallback.

The preserved WIP regression demonstrates the previous mismatch: an opaque
root object containing tools was skipped by collection but visited by
replacement, consuming the mask intended for input. Both local and remote
masking now apply the original compiled selection. Replacement count changes
are rejected before mutation. Rust retains Serde acquisition, wire decoding,
validated mutable node access and error propagation.

The ABI uses the existing measured JSON protocol and 64 sixteen-byte
caller-owned scratch units for display paths. A known key requires at most
13 bytes and an array index 22; the 32-level inspection limit keeps paths
inside the 1,024-byte buffer. The object has no undefined symbols, including
no Mojo allocator/runtime dependencies. ABI or acquisition failure returns
an error instead of recomputing policy in Rust.

The production build selects and links the new source through mojo-runtime.
The official unchanged metric rises from 35.172814243880104% at 12c23df4d
to 35.215954263254275%: 126,152 Mojo and 232,072 Rust production lines.
The remaining gap to 75% is 39.784045736745725 percentage points.

Validation from the repository root:

- PASS: `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/redaction_json_test.mojo`.
- PASS (2 tests): `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::redaction_content::tests`.
- PASS (33 tests): `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-app --lib runtime_proxy::presidio:: -- --test-threads=1`.
- PASS: `node scripts/ci/test-serial.mjs --suite runtime-smoke`.
- PASS: `node scripts/ci/runtime-test-manifest-guard.mjs` (504 listed tests).
- PASS: `node scripts/ci/runtime-hotpath-guard.mjs --self-test && node scripts/ci/runtime-hotpath-guard.mjs`.
- PASS: `node scripts/ci/crate-boundary-guard.mjs --self-test && node scripts/ci/crate-boundary-guard.mjs`.
- PASS: `node scripts/compat/check-upstream-baseline.mjs && node scripts/compat/capture-replay-fixture-tests.mjs`.
- PASS: `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`, `node scripts/ci/mojo-no-fallback-guard.mjs`, `node scripts/ci/mojo-authority-guard.mjs`.
- PASS: `node scripts/ci/mojo-production-share.mjs --check --json`, `cargo fmt --all --check`, `npm run docs`, `git diff --check`, staged churn hygiene.
- FAIL: `npm run test:changed` stops on the existing 860-line deepseek_sse_tests.rs and 33 near-limit files (budget 32).
- FAIL: `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` stops on existing chunks_exact_to_as_chunks in anthropic_stream_policy.rs and items_after_test_module in provider_constraints/gemini_bridge_request.rs.

The broad proxy check subsequently passed all 459 tests:
`cargo test --locked -q -p prodex-app --lib 'main_internal_tests::runtime_proxy_' -- --test-threads=1`.
No guard was weakened; full CI and native macOS/Windows execution were not run.
