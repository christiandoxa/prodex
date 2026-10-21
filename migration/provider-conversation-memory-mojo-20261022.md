# Provider conversation memory estimation in Mojo (2026-10-22)

## Production ownership

`provider_conversation_memory.mojo` now owns the cache's complete allocation
estimate: key bytes, one host Value size per JSON node, boolean and number
payload sizes, decoded UTF-8 string/key lengths, and saturation at the host's
unsigned pointer limit. The synthetic messages array contributes no Value
allocation. This preserves the previous estimate rather than introducing a
different accounting formula.

The application store calls the provider-core Serde adapter, which reuses
`Document::borrowed_array`. It reuses the existing traversal without adding a
parser or serializing or copying string payloads. Rust supplies representation
sizes and numeric node facts;
Mojo receives no message contents. The old recursive Rust estimate and its
saturating sums are deleted without a fallback. Invalid ABI results leave
cache state untouched.

The versioned numeric ABI validates count, record/output alignment, layout
sizes, the synthetic root and all node tags before writing. The 24-byte row
count bound ensures the full nonnegative sum fits UInt128; the final clamp
therefore equals the old nested saturating additions. Caller-owned facts and
the existing borrowed shape use linear storage. Large strings remain borrowed
and require only byte-length facts across the ABI.

## Verification

Direct Mojo assertions check exact accounting, the synthetic root, full-width
and 32-bit saturation, version/shape rejection and atomic failure. Compiled
bridge tests check the checked numeric boundary. Provider-core and production
store tests independently count nested JSON, all JSON kinds and Unicode keys.
The store's real oversized replacement and per-scope/global retention tests
exercise the estimator through the production cache path.

Compiled Cargo commands use Mojo 1.1.0 with `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`. The changed-test runner keeps its default features.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/provider_conversation_memory_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::provider_conversation_memory::`
- `cargo test --locked -q -p prodex-provider-core --lib`
- `cargo test --locked -q -p prodex-app --lib conversation_store -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_launch::proxy_startup:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib 'main_internal_tests::runtime_proxy_' -- --test-threads=1`
- `node scripts/ci/test-serial.mjs --suite runtime-smoke`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/runtime-hotpath-guard.mjs --self-test`
- `node scripts/ci/runtime-hotpath-guard.mjs`
- `node scripts/ci/runtime-test-manifest-guard.mjs`
- `node scripts/ci/crate-boundary-guard.mjs --self-test`
- `node scripts/ci/crate-boundary-guard.mjs`
- `node scripts/ci/provider-spi-boundary-guard.mjs --self-test`
- `node scripts/ci/provider-spi-boundary-guard.mjs`
- `node scripts/compat/check-upstream-baseline.mjs`
- `node scripts/compat/capture-replay-fixture-tests.mjs`
- `npm run docs`
- `npm run test:changed`
- `npm run ci -- --jobs 1`
- `git diff --check`

Live provider and native non-Linux execution remain outside this offline
checkpoint. The 75% project goal remains active.

Direct Mojo assertions, two strict bridge tests, all 535 provider-core tests,
six caller-filter tests (including all five dedicated store tests), all 550
provider/startup tests and all 459 proxy tests pass. Runtime-smoke, strict
workspace Clippy, the runtime manifest, no-fallback self-test, authority,
ownership, source-share, size, hot-path, crate/provider boundary and offline
compatibility checks pass. Canonical source share is 36.304688936071585%
(136,278 Mojo / 239,095 Rust production LOC).

The full local CI gate and default changed-test runner pass. CI passes 282 Node
tests and skips its native Windows installer test on Linux. Feature-off bridge
runs are not counted as ABI evidence; the two strict compiled tests above ran
and passed. An initial command used a nonexistent provider-core guard path;
the actual provider SPI guard and its self-test were then run successfully.
