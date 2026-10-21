# Provider conversation selection and retention in Mojo (2026-10-21)

## Production ownership

`provider_conversation_policy.mojo` owns scope eligibility, latest matching
history, replacement, sequence saturation, per-scope eviction, global entry
and byte limits, and retained-byte accounting. The real shared provider store
uses its compiled plans. Rust's scope matcher, latest-sequence selection,
oldest-entry loops and saturating retention counters are deleted without a
Rust fallback. DeepSeek, Gemini, Kiro and compatible chat callers share this
store; network, synchronization, BTreeMap mutation and Serde values stay in Rust.

The host supplies unique borrowed rows in BTreeMap key order. The v1 ABI
validates row counts, pointer shape/alignment, UTF-8, key ordering, byte totals,
sequence consistency and output capacity before mutation. At most 2048 old
rows and one incoming row enter a retention plan. Caller-owned scratch uses the
existing index heapsort only when eviction is necessary; ordinary inserts are
linear. Returned masks apply directly to the original host-owned values.

History selection first obtains a Mojo scope mask. Rust invokes the opaque
predicate only for eligible rows, in original key order. Mojo then selects the
latest matching sequence, retaining the later lexical key on equal sequence.
Eviction uses the earliest lexical key on equal sequence. Oversized replacement
removes the prior value without advancing the counter. Scope eviction precedes
global eviction; sequence saturation can evict the incoming value itself.

Existing scope-key rendering and recursive Serde memory estimation are outside
this retention checkpoint. They do not recompute any promoted decision.

## Verification

Direct Mojo assertions cover scope masks, history ties, replacement accounting,
oversized replacement and atomic rejection of wrong versions, capacity, pointer
shape, row count, invalid masks and malformed UTF-8. Compiled bridge tests also
cover sequence saturation, lexical eviction ties, exact byte limits, global
entry pressure, global memory pressure and malformed ordered facts. Production
caller tests exercise scope isolation, predicate order, live per-scope/global
retention, replacement accounting and a genuinely oversized Serde value.
The caller tests are included in the runtime-smoke manifest.

Commands use Mojo 1.1.0. Compiled Cargo checks set `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`; the changed-test runner keeps its default features.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/provider_conversation_policy_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib provider_conversation_policy::`
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
- `node scripts/compat/check-upstream-baseline.mjs`
- `node scripts/compat/capture-replay-fixture-tests.mjs`
- `npm run docs`
- `npm run test:changed`
- `npm run ci -- --jobs 1`
- `git diff --check`

Live providers and native non-Linux runs are outside this offline checkpoint.
The 75% project goal remains open.

Direct Mojo assertions, five compiled bridge tests, five caller-filter tests
(including all four dedicated store tests), all 549 provider/startup tests,
all 459 runtime proxy tests, runtime-smoke and strict workspace Clippy pass.
The runtime manifest validates 505 discovered internal tests. Ownership,
authority, no-fallback self-test, source-share, size, crate-boundary,
hot-path, secret-boundary and offline compatibility checks pass.

Canonical production share is 36.30450538791955% (136,245 Mojo / 239,039 Rust
production LOC). The target remains 75%.

The full local CI gate and default changed-test runner pass. CI's native Windows
installer test is skipped on Linux; 282 other Node tests pass. Its first attempt
caught a new wildcard import in the extracted store tests. Explicit imports
fixed the failure without changing the wildcard cap. Feature-off zero-test
bridge runs in the changed runner are not counted as compiled ABI coverage;
the five strict bridge tests above provide that evidence.
