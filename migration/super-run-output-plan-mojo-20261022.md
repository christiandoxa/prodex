# Super run output append and marker plans in Mojo (2026-10-22)

## Production ownership

Mojo now plans one redacted output chunk in full: event byte prefix, aggregate
append prefix, sticky truncation, normal event mutation and optional truncation
marker mutation. The Rust remaining-capacity arithmetic, truncation branches
and marker-existence decision are removed. Rust retains lossy decoding,
redaction, existing compatibility byte budgets, string buffers and deque
mutation. Its shared event applier performs storage effects for the already
compiled plan; it makes no retention or sequence decisions.

The new versioned ABI uses an 80-byte input record, borrowed UTF-8 and numeric
marker flags, plus nine fixed-width output words. It validates version,
alignment, signed byte/count bounds, all boolean flags and UTF-8 before any
output write. Rust verifies output tags, byte boundaries and drop counts
before mutating storage. ABI errors preserve the buffer, events and sequence;
there is no Rust output policy fallback.

The existing event insertion kernel is extracted as one pure function. Its
unchanged v1 C facade and output planner share sequence saturation and oldest
event eviction. The existing UTF-8 floor-cut kernel supplies both prefixes.
The plan checks marker presence after the normal event's eviction, matching
the original order. A marker evicted by that insertion is emitted again; a
retained marker or a marker-typed new chunk suppresses a duplicate. Empty text
leaves all state untouched, and output events continue after the aggregate cap.

The new retention test's Rust policy oracle is also deleted. Fixed expected
eviction indices cover the same input sizes and full-width timestamps, so
the test no longer retains a second eviction implementation.

## Verification

Direct Mojo assertions cover multibyte prefixes, empty text, saturated
sequences, marker eviction/retention, marker-typed chunks and atomic late-flag
rejection. Compiled bridge tests cover event-only clipping and full-width
buffer facts. Production output tests verify exact deque contents and
sequence counts, empty chunks, marker deduplication, lossy UTF-8, secret
redaction and event/aggregate byte limits. Existing run, protocol, clipping
and override tests run with the same expanded bridge.

Strict Cargo checks use Mojo 1.1.0 with `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`; default CI and changed-test selection omit them.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/super_run_output_test.mojo`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/super_run_event_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib rich::super_expose::run_policy::`
- `cargo test --locked -q -p prodex-app --lib super_expose::run::output_tests:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib super_expose:: -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/crate-boundary-guard.mjs --self-test`
- `node scripts/ci/crate-boundary-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `npm run docs`
- `npm run test:changed`
- `npm run ci -- --jobs 1`
- `git diff --check`

Native non-Linux, live-provider and full-workspace tests remain outside this
offline unit. The 75% project target remains open.

The first Clippy pass rejected an unused fixture assignment after its pointer
had been stored as an ABI integer. Refreshing that input address after mutation
makes the raw-array lifetime/use explicit; no production change was needed.

Both direct Mojo programs, all nine strict run-policy bridge tests, all three
output regressions and all 116 Super expose tests pass. Authority, ownership,
no-fallback self-test, size, crate-boundary self-test/check and secret-boundary
checks pass. Canonical share is 36.31228157896137% (136,532 Mojo / 239,462 Rust
production LOC). Checked record decoding and host mutation adapters slightly
lower raw share; counting rules and guard thresholds are unchanged.

Strict workspace Clippy, final documentation/whitespace checks, the full local
CI gate and changed-test runner pass. CI includes 685 runtime-launch tests,
43 auto-rotate tests and 282 passing Node tests; its Windows installer test is
skipped on Linux. Feature-off zero-test core harnesses in changed-test selection
are not counted as compiled ABI coverage.
