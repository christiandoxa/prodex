# Shared catalog kernels retain their C contracts (2026-10-22)

Existing catalog merge and Copilot prompt-limit decisions now have typed internal
Mojo entrypoints. Their original versioned C exports delegate to those owners.
The loop, trim/case rules, limits, status and partial-count behavior are retained.
No Rust algorithm moves in this preparation; the following catalog plan reuses
these kernels without importing duplicate C exports or a stack count address.

The owned feature change exceeded the 1,200-line churn threshold. This small
extraction is published first; the feature WIP and its twelve original catalog
baselines remain protected. The threshold and counting rules are unchanged.

Direct Mojo, one core scalar, eight runtime catalog, 29 rich catalog, 552 provider
and 19 application caller tests pass. Strict workspace Clippy and formatting pass.
Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`. Commands:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/catalog_shared_kernels_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib provider_scalar_policy`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib catalog`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-rich --lib catalog`
- `cargo test --locked -q -p prodex-provider-core -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_external_provider_config:: -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`

Guards, offline compatibility, docs, changed tests and whitespace use the unchanged
[validation matrix](external-model-schema-mojo-20261022.md). Full default CI runs
with the immediately following complete catalog-plan checkpoint. The 75% target
remains open; native non-Linux, live providers and performance are not covered.

`cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib provider_scalar_policy`
selected zero tests and was rejected by the runner (99); the runtime catalog
boundary supplies its available feature coverage.

`cargo test --locked -q -p prodex-mojo-core --features mojo-rich catalog` failed
with E0432 at the pre-existing runtime-log integration test gate requiring
`mojo-routing`. Its Rust library/manifest are
unchanged from the parent; focused library tests and all-feature Clippy validate
the extraction. The failed command and diagnostic remain in evidence.

Canonical share is 36.6340713504604% (138,690 Mojo / 239,892 Rust production
LOC). This preparation changes reuse boundaries without transferring new Rust
semantics. Default changed tests pass with zero Rust tests selected; explicit
bridge/caller coverage above supplies those checks. Docs, staged churn and
whitespace checks pass.
