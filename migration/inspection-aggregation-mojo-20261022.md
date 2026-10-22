# Runtime inspection aggregation in Mojo (2026-10-22)

`runtime_presidio_inspection_plan` now uses compiled coverage combination,
classification escalation, source-local mask intersection and ordered unique
mask union. Rust's aggregation loops, membership searches, sort and dedup are
removed without a fallback. Only numeric coverage/kind/count metadata crosses
the ABI. Original finding locations, tags, reasons and detector revision stay
with Rust and the existing validated domain constructor.

The existing coverage facade and aggregate share one pure coverage kernel.
Aggregate count checks reuse the domain record-count kernel before reading
finding payloads, preserving the original `LimitExceeded` error and context
even for very large invalid inputs. Source-count and per-source-mask guards
retain their original precedence before result-count errors. Empty sources
keep unsupported coverage and the supplied default classification.

The versioned rich-boundary ABI accepts eight sources and twelve mask tags per
source. Successful plans are bounded by the existing 256-finding and 32-tag /
32-reason limits. Validation rejects unknown tags, negative counts, wrong
versions, null/length and alignment errors without partial output. The host
checks returned classification, coverage and ordered unique mask tags without
recomputing policy. No dependency, counting change or Rust fallback is added.

Strict Cargo sets `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`;
default CI/changed tests omit both and any archive override. Direct assertions
use `-D ASSERT=all`. Validation commands:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/inspection_aggregation_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib policy::inspection_aggregation::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib policy::inspection_aggregation::`
- `cargo test --locked -q -p prodex-domain --test governance_inspection`
- `cargo test --locked -q -p prodex-app --lib runtime_proxy::presidio::findings::aggregation_tests:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_proxy::presidio:: -- --test-threads=1`
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
- `node scripts/ci/super-wildcard-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `node scripts/ci/churn-hygiene.mjs --staged --check`
- `npm run ci -- --jobs 1`
- `npm run test:changed`
- `npm run docs`
- `git diff --cached --check`
- `git diff --check`

Six caller regressions pass before and after migration with unchanged
expectations. Direct Mojo, five bridge tests in each feature configuration,
seven domain, 44 Presidio, 459 application proxy and 19 serialized smoke tests
pass. Strict workspace Clippy, formatting, runtime manifest and all guards
pass. Full default CI passes in 530.10 seconds with 1,443 Rust tests; changed
tests pass in 6.57 seconds, and documentation passes. Feature-off zero-test
harnesses do not replace strict positive ABI/caller evidence. Evidence retains a direct-test
integer type correction, missing fixture commas and a test allocation rejected
by Clippy. Corrected fixtures use arrays and normal formatting. Canonical share is
36.40382983237853% (137,257 Mojo / 239,783 Rust production LOC). Checked host
transport slightly reduces raw share while transferring semantic authority.
The 75% target remains open. Native non-Linux execution, live providers and
exhaustive workspace/all-serialized suites remain outside local coverage.
