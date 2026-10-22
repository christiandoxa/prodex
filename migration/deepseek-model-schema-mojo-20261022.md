# DeepSeek runtime model schema compiled in Mojo (2026-10-22)

The existing compiled runtime-catalog renderer now owns the DeepSeek schema,
its four fixed reasoning descriptions, high default, text-only capabilities and
430-byte base instructions. The old Rust JSON object and private instruction
constant are deleted without a fallback. Shared schema fields reuse the same
owner as Copilot/Kiro/Anthropic. Instruction literals use explicit LF escapes
so source checkout line endings do not alter the value.

Rust retains provider metadata lookup/fallback, candidate ordering and compiled
ID merge, Serde acquisition/decoding, config and secure filesystem writes.
Renderer errors propagate through the existing model/list/writer Results; the
writer unwraps the list Result before serialization. Existing static metadata
and ID-merge failure behavior is unchanged. No dependency, counting rule,
public CLI/config contract or guard cap changes.

The version-1 measured JSON ABI and its validation are reused unchanged. Direct
and bridge tests extend existing provider coverage. Two complete original Rust
objects are frozen in `runtime_deepseek_config/schema_baseline.json`, covering
Unicode/escaping, zero and unsigned endpoints. The frozen caller passes before
migration. Its fixture SHA256 is
`bcb8a14d995e025f72342971ad671379695851c9c25aeef80e6f8e873eb63c5b`;
instruction SHA256 is
`d3f1df2e5af5e0088df0b63a6ce791bdad9bbe4c79efde76d1f49424c98ee97f`.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`.
Default CI/changed tests omit both and any archive override. Commands:

- `cargo test --locked -q -p prodex-app --lib deepseek_runtime_schema_preserves_original_catalog_objects -- --test-threads=1 --nocapture`
- `cargo test --locked -q -p prodex-app --lib deepseek_runtime_schema_preserves_original_catalog_objects -- --test-threads=1`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_model_catalog_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::runtime_catalog::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::runtime_catalog::`
- `cargo test --locked -q -p prodex-app --lib runtime_deepseek_config:: -- --test-threads=1`
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

Direct Mojo, two bridge tests in each feature mode, five DeepSeek caller tests,
17 external configuration tests, 550 provider tests, strict workspace Clippy,
formatting, source/ownership/boundary guards and offline compatibility pass.
Full default CI passes 1,443 Rust tests in 551.04 seconds, with its existing
native Windows installer test skipped on Linux. Default changed tests pass
550 Rust tests in 108.41 seconds; their no-feature app/core invocations select
zero tests, covered by the explicit feature/caller commands above. Documentation,
staged churn and whitespace checks pass. An initial caller
compile rejected an import through the private catalog module; the adapter now
uses the existing public crate-root exports. The failed structured check and
harvested diagnostic excerpt remain in evidence.

Canonical share is 36.6176680386987% (138,603 Mojo /
239,911 Rust production LOC). Rust production source decreases by
46 LOC. The 75% goal remains open. Native non-Linux execution, live providers, exhaustive
workspace/all-serialized suites and performance benchmarks remain outside coverage.
