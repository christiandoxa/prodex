# Native recovery wire state compiled in Mojo (2026-10-22)

Goal/thread status classification, absent versus unknown goal state, optional
token-budget handling and signed budget comparisons now belong to the existing
compiled launch-recovery owner. The wire entry calls the unchanged version-2
in-place action policy. Rust's status matches and budget decision are deleted
without a fallback or a second production decision.

Rust retains Serde field/type acquisition, including `as_i64` rejection of
floats and out-of-range unsigned values. Named input fields marshal those facts
through two existing borrowed string views and scalar presence/type flags.
Mojo validates the ABI version, exact view count, pointer alignment, flags,
view representation, lengths and UTF-8. It owns missing/null unlimited budgets
and the `limit > 0`, `used >= 0`, `used < limit` checks. No new input limit is
introduced. The adapter reuses the existing action decoder and string marshaller.

Native home/thread/workspace identity, socket/RPC order, affinity release,
mutation confirmation, delivery and timing remain host responsibilities.
Source comparison proves the identity body, host recovery prefix and mutation/
socket recovery tail unchanged. All pre-existing launch-config Mojo bytes,
including the typed recovery policy, are unchanged. The ownership manifest's
consumer marker follows the real new call; counting rules and guards do not
change.

Two new local app-server caller regressions, within an 11-test suite, pass on
the original Rust implementation. They preserve exact status spelling, missing
and malformed fields, unlimited missing/null budgets, signed integer endpoints,
invalid scalar/container types and overflow. Existing protocol tests cover user
stops, active work, wrong identity, same-thread continuation, rejection and
ambiguous delivery without duplicate mutation.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`.
Default CI/changed tests omit both variables and any archive override. Commands:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/launch_native_recovery_wire_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib launch::recovery::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib launch::recovery::`
- `cargo test --locked -q -p prodex-app --lib runtime_tools::session_recovery::tests:: -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/test-serial.mjs --suite runtime-smoke`
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

Direct Mojo, 14 bridge tests in each feature mode, all 11 before/after caller
tests, strict workspace Clippy and formatting pass. Runtime smoke passes 19 tests
in 91.95 seconds. Full default CI passes 1,443 Rust tests in 493.38 seconds.
Default changed tests pass in 2.53 seconds but execute zero Rust tests in these
no-feature crates; the explicit feature bridge and application commands above
supply that coverage. Source/ownership guards, documentation, staged churn and
whitespace checks pass. Evidence retains the corrected Mojo loop-index type error
and the ownership marker failure preceding its real-caller update.

Canonical share is 36.54822871858054% (138,236 Mojo / 239,993 Rust production
LOC). The checked wire adapter adds 20 Rust production LOC while transferring
decisions. The 75% target remains open. Native non-Linux execution, live
providers, exhaustive workspace/all-serialized suites and performance benchmarks
remain outside local coverage.
