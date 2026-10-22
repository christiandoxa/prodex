# External runtime model schemas compiled in Mojo (2026-10-22)

The Copilot/Kiro/Anthropic runtime model schema, reasoning defaults, eight effort
descriptions, Unknown filtering and provider capability/modalities now belong to
the compiled runtime-catalog owner. Rust's schema object, default effort array,
description match and capability decisions are deleted without a fallback.
Existing provider metadata acquisition is unchanged and passes optional fields
without choosing defaults. Serde acquisition/decoding remains in the provider
adapter; existing config/filesystem effects remain in the application.

The version-1 JSON ABI reuses the parsed-tree validator, unsigned parser, measured
JSON writer and host allocator. Versions, flags, alignment, counts, UTF-8, tree
links, raw spans, typed fields, effort labels, phase and capacity are validated.
Insufficient output capacity leaves output/metadata unchanged. Missing effort
metadata defaults as before; explicit empty arrays remain empty, Unknown entries
are omitted and unknown default metadata selects high. All unsigned values,
including zero and u64::MAX, retain their representation.

The private renderer propagates typed bridge errors through the existing config
Result instead of introducing a production panic. Public CLI/config contracts
and successful values are unchanged. Four complete objects captured by the
original Rust caller are frozen in `catalog_model/schema_baseline.json`, including
Kiro's fifth max-effort level, Unicode/escaping and unsigned endpoints. The frozen
caller passes before migration and compares complete objects after migration.
Dynamic catalog parsing, candidate order, context arithmetic and the entire
DeepSeek source are unchanged; DeepSeek presentation is the next checkpoint.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`.
Default CI/changed tests omit both and any archive override. Commands:

- `cargo test --locked -q -p prodex-app --lib external_runtime_schema_preserves_original_catalog_objects -- --test-threads=1 --nocapture`
- `cargo test --locked -q -p prodex-app --lib external_runtime_schema_preserves_original_catalog_objects -- --test-threads=1`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_model_catalog_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::runtime_catalog::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::runtime_catalog::`
- `cargo test --locked -q -p prodex-app --lib runtime_external_provider_config:: -- --test-threads=1`
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

Direct Mojo, two bridge tests in each feature mode, the frozen before/after
caller, 17 config callers, 549 provider tests, strict workspace Clippy, formatting,
guards and offline compatibility pass. Full default CI passes 1,443 Rust tests
in 594.20 seconds. Default changed tests pass 549 Rust tests in 92.52 seconds;
their no-feature app/core invocations run zero tests, supplied instead by explicit
feature/caller commands above. Documentation, staged churn and whitespace pass.
Initial CI stopped at the wildcard-import
ratchet; the new application test now uses explicit imports and its guard/caller/
Clippy checks pass without changing the cap. Evidence also retains the corrected
prototype StringSlice return-type constraint; static rich-string views reuse the
established literal representation.

Canonical share is 36.610706785190665% (138,588 Mojo / 239,957 Rust production
LOC). The checked provider/ABI adapter adds 21 Rust production LOC while removing
its former semantic ownership. No dependency, counting rule or guard changes.
The 75% target remains open. Native non-Linux execution, live providers, exhaustive
workspace/all-serialized suites and performance benchmarks remain outside coverage.
