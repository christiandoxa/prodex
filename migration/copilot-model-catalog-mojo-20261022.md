# Copilot runtime model catalogs compiled in Mojo (2026-10-22)

Recursive model-list discovery, raw-entry limits, field precedence, Unicode trim,
unsigned context/prompt limits, normalized model JSON and recursive object-null
removal now belong to a compiled Copilot catalog owner. Five Rust semantic
functions are deleted without a fallback. The existing compiled ID merge and
provider catalog validation remain in use through the provider-owned adapter.
Credentials, token/model HTTP requests and their effect/error order are unchanged.

The version-1 JSON ABI reuses the existing Serde tree adapter, measured transform
allocator, parsed-tree validator, JSON writer, subtree traversal and unsigned
decimal parser. It validates versions, counts, flags, alignment, UTF-8, node links,
raw spans, phase and capacity. A measurement pass prevents insufficient-capacity
writes to output or metadata. A standard serialized Result carries the normalized
model array or the exact existing entry-limit diagnostic; Rust only decodes it.
No dependency, counting rule, source exclusion or guard changes are introduced.

Matched arrays preserve traversal order and stop descent into their entries.
All entries count toward the unchanged 1,024 limit before invalid filtering and
deduplication. Present null/malformed earlier aliases block later aliases.
Prompt limits precede total context limits; without a usable limit, context
defaults to 200,000. Object null fields disappear recursively while null array
elements keep their positions. Negative/float rejection and full u64 values are preserved.
The provider adapter retains typed errors and validates returned ID/index access.

Four new caller regressions pass against the original Rust implementation.
They compare complete model objects, traversal/dedup order, malformed and Unicode
fields, unsigned endpoints, alias shadowing, null pruning and raw-count limits.
Direct Mojo and two bridge tests in each core/runtime feature mode pass, followed
by all 547 provider tests and 13 local catalog/auth caller tests. The source proof
preserves every credential/HTTP byte except removed unused imports.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`.
Default CI/changed tests omit both variables and any archive override. Commands:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/copilot_model_catalog_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::copilot_catalog::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::copilot_catalog::`
- `cargo test --locked -q -p prodex-provider-core -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib profile_commands::copilot::tests::runtime_catalog:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib profile_commands::copilot::tests:: -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
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

Strict workspace Clippy, formatting, guards and offline compatibility pass.
Full default CI passes 1,443 Rust tests in 577.71 seconds. Final changed-test/
documentation checks pass; default changed tests execute 547 Rust tests in
87.73 seconds. Their no-feature app/core invocations execute zero tests; the
explicit bridge/caller commands supply that coverage. Evidence
retains the corrected Mojo byte-conversion and compile-time null-pointer errors;
measurement now uses an undereferenced live scratch pointer.

Canonical share is 36.58358662613982% (138,414 Mojo / 239,936 Rust production
LOC), removing 57 Rust production LOC. The 75% target remains open. Native
non-Linux execution, live providers, exhaustive workspace/all-serialized suites
and performance benchmarks remain outside local coverage.
