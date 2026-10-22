# Stored runtime model catalogs projected in Mojo (2026-10-22)

The compiled runtime-catalog owner now validates the models array and raw-entry
limit, selects presence-first id/slug/model and name/model_name fields, trims
Unicode model IDs and projects runtime metadata. It preserves input order and
duplicates, untrimmed display/description strings, optional fields and the exact
unsigned prompt/context precedence. A present invalid prompt field blocks the
nested alias; unsigned context-token values zero/one block the context alias
before the final greater-than-one check.

The original Rust filtering/selection code and prompt-limit helper are deleted
without a Rust fallback. The private application record moves to the provider's
typed decoded output. Rust retains bounded file reads, Serde acquisition and
decoding, provider path selection and existing Kiro normalization. Existing
candidate/context/compact assembly and schema mode remain unchanged. Compiled
rejections propagate their original messages through the existing Result.

Stored-catalog mode extends the existing version-1 measured JSON ABI, with a
nonnegative raw-entry bound. Existing valid schema-mode calls keep their contract.
Unknown modes, negative bounds, versions, pointers/alignment, counts, UTF-8, tree
links, raw spans, phases and capacity are checked. Insufficient capacity leaves
output and metadata unchanged. The JSON writer, unsigned parser and Unicode trim
are reused. No dependency, guard cap or counting rule changes.

Two caller regressions pass on original Rust before migration, then pass unchanged
inside all 19 external configuration tests. They cover complete objects, exact
rejections, limits before filtering, aliases, raw text, invalid numeric types,
u64 endpoints and duplicates. Both direct Mojo tests, three bridge tests in each
core/runtime mode, five DeepSeek callers and 552 provider tests pass. Strict
workspace Clippy, formatting, source/ownership/boundary guards and offline
compatibility pass. Full default CI passes 1,443 Rust tests in 571.39 seconds,
with its existing native Windows installer skip on Linux. Default changed tests
pass 552 Rust tests in 96.97 seconds; their no-feature app/core invocations select
zero tests, covered by the explicit feature/caller commands above. Documentation,
staged churn and whitespace checks pass.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`.
Default CI/changed tests omit both and any archive override. Commands:

- `cargo test --locked -q -p prodex-app --lib dynamic_catalog_projection_ -- --test-threads=1`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_dynamic_catalog_test.mojo`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_model_catalog_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::runtime_catalog::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::runtime_catalog::`
- `cargo test --locked -q -p prodex-app --lib runtime_external_provider_config:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_deepseek_config:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-provider-core -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `node scripts/ci/super-wildcard-guard.mjs`
- `node scripts/ci/crate-boundary-guard.mjs --self-test`
- `node scripts/ci/crate-boundary-guard.mjs`
- `node scripts/ci/provider-spi-boundary-guard.mjs --self-test`
- `node scripts/ci/provider-spi-boundary-guard.mjs`
- `node scripts/ci/application-boundary-guard.mjs --self-test`
- `node scripts/ci/application-boundary-guard.mjs`
- `node scripts/compat/check-upstream-baseline.mjs`
- `node scripts/compat/capture-replay-fixture-tests.mjs`
- `node scripts/ci/churn-hygiene.mjs --staged --check`
- `npm run ci -- --jobs 1`
- `npm run test:changed`
- `npm run docs`
- `git diff --cached --check`
- `git diff --check`

An initial prototype test rejected an implicit Int-to-Int64 comparison; an
explicit cast corrected it. Both logs remain in evidence. Its corrected direct
test passes before production adoption; no production decision changed to satisfy
the test.

Canonical share is 36.63089102738514% (138,671 Mojo /
239,892 Rust production LOC), removing 19 Rust production LOC.
The 75% goal remains open. Native macOS/Windows, live providers, exhaustive
workspace/all-serialized suites and performance benchmarks remain outside coverage.
