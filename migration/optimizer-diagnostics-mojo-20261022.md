# Optimizer preflight diagnostics in Mojo (2026-10-22)

Optimizer shape and required-MCP validation now obtain exact diagnostic text
from compiled Mojo. Both Rust message tables are removed without a fallback.
Existing policy and precedence calls remain unchanged: disabled/nonboolean
enabled flags still reject before command resolution, and shape preflight
still precedes activation writes. The host retains TOML/filesystem acquisition
and propagates errors; Mojo owns eight shape messages and three required-MCP
templates with the original server name.

The version-1 ABI accepts a diagnostic group, an explicit issue tag and a
borrowed UTF-8 name, then writes to bounded caller-owned output. Existing public
issue enums have explicit numeric tags; their variants and validation APIs
remain compatible. Input tags, name shape/UTF-8, alignment and capacity validate
before output mutation. An internal measurement pass preserves output/length
sentinels on failure. The focused diagnostic module extends the existing
optional-tools owner without growing its large Mojo policy file. No dependency,
Rust fallback, duplicate message table or counting-rule change is added.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`.
Default changed tests omit both and any archive override. Commands:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/optional_tool_diagnostics_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib optional_tools_policy::diagnostics::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib optional_tools_policy::diagnostics::`
- `cargo test --locked -q -p prodex-optional-tools --lib super_optimizers::diagnostic_tests:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-optional-tools -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib optional_tools -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `node scripts/ci/churn-hygiene.mjs --staged --check`
- `npm run test:changed`
- `npm run docs`
- `git diff --cached --check`
- `git diff --check`

Two caller regressions pass before/after migration, with direct Mojo, three
bridge tests in each feature configuration, 71 optional-tools and 18 app tests.
Strict workspace Clippy, formatting, ownership, authority, no-fallback, size
and secret guards pass. Changed tests pass in 90.36 seconds with 71 Rust tests;
documentation, staged churn and diff checks pass. Evidence retains
the initial integer-width and generic string-view return corrections. Canonical
share is 36.447182333903626% (137,618 Mojo / 239,964 Rust production LOC).
The 75% target remains open. The prior report checkpoint passed full default CI;
this diagnostic-only change uses focused suites and strict workspace Clippy.
Native non-Linux execution, live providers and exhaustive workspace/serialized
suites remain outside local coverage.
