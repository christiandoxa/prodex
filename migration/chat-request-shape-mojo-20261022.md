# Chat-compatible request shape validation in Mojo (2026-10-22)

The compiled chat-request validation owner now checks root-object shape,
optional model type and Unicode blankness, stream booleans, instruction strings
and previous-response strings in their original order. Rust's six rejection
branches and diagnostic templates are deleted without a fallback. Absent fields,
empty optional strings and exact provider-labelled errors retain their behavior.

Operation 1 extends the existing version-1 measured JSON ABI. It borrows request
nodes and the provider label; the existing Serde acquisition helper avoids
serializing or cloning request bodies. Mojo owns all decisions and diagnostic
text. The adapter validates result tags, UTF-8 and output shape. Invalid versions,
flags, links, pointers, alignment and capacity are rejected before publishing
shape output. Operation 0 retains its existing transformation contract. Existing
request wrappers move into a focused private module with unchanged public
re-exports. No dependency or counting-rule changes are added.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`.
Default CI/changed tests omit both and any archive override. Commands:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/chat_request_shape_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::openai_chat_request::tests::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::openai_chat_request::tests::`
- `cargo test --locked -q -p prodex-provider-core --lib bridge::chat_compat::shape_tests:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-provider-core -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_launch::proxy_startup:: -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/compat/check-upstream-baseline.mjs`
- `node scripts/compat/capture-replay-fixture-tests.mjs`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `node scripts/ci/churn-hygiene.mjs --staged --check`
- `npm run ci -- --jobs 1`
- `npm run test:changed`
- `npm run docs`
- `git diff --cached --check`
- `git diff --check`

Two caller regressions pass before/after migration, covering all Rust whitespace
characters, Unicode non-whitespace, NUL, large models, optional fields and exact
multi-error precedence. Direct Mojo, three bridge tests in each feature
configuration, 547 provider-crate tests and 551 application provider callers pass,
with strict workspace Clippy and ownership/compatibility guards.
Full default CI passes in 493.80 seconds with 1,443 Rust tests. Changed tests
pass in 75.85 seconds with 547 Rust tests. Documentation, staged churn and diff
checks pass.

Evidence retains compiler corrections in prototype fixtures, one intentionally
cancelled overlapping Cargo invocation followed by its complete serialized rerun,
and an initial minimum-count mismatch: the narrower caller filter selects 551
passing tests, while the full runtime-launch filter selects 685. No failed test
was hidden or converted into a passing result.

Canonical share is 36.47306475315425% (137,746 Mojo / 239,919 Rust production LOC).
The 75% target remains open. Native non-Linux execution, live providers,
exhaustive workspace/all-serialized suites and performance benchmarks are outside
local coverage.
