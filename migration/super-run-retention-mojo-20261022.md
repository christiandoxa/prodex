# Super completed-run retention in Mojo (2026-10-22)

## Production ownership

`super_expose_run_policy.mojo` now owns the completed-run count, fixed limit of
32, finished-or-created timestamp precedence, creation-time tie break, stable
lexical ID tie break and complete retention mask. The Rust count/filter/minimum
loop and its cap constant are deleted without a fallback. All four production
completion/cancellation paths apply the shared mask to their original BTreeMap.
The existing run-state classification supplies terminal facts; locks, IDs,
child handles, output/event storage and process effects stay with Rust.

The existing signed binding-rank adapter cannot represent the full unsigned
run timestamps without changing their order. This focused Super domain owner
instead reuses the common caller-owned index heapsort with unsigned fields.
It sorts only under terminal pressure, puts eligible rows first, and uses
the original BTreeMap index for exact time ties. Active, starting and queued
records survive regardless of their age or the number of retained terminal
records. All 64 bits of each timestamp remain available.

The v1 ABI validates the count/record-size bound, alignment, boolean tags,
canonical optional timestamps and both caller-owned capacities before any
output or scratch mutation. Rust validates mask tags before touching storage.
An ABI error preserves all records and does not recompute eviction in Rust.
No run IDs, prompts, output text or credentials cross the numeric ABI.

Wiring the focused regression module brought `run.rs` to the repository's
770-line cohesion threshold. Existing inline tests were moved intact to
`run/tests.rs`, keeping their module paths and widening only a test-fixture's
private visibility for the sibling retention tests. The production composition
file is now 582 lines; size thresholds and allowlists are unchanged.

## Verification

Direct Mojo assertions cover active protection, full-width unsigned times,
finished-time fallback, the fixed cap, empty input and atomic ABI rejection.
Three compiled tests also compare masks with a test-only original-policy
oracle across several input sizes and verify stable ties and malformed rows.
Production caller tests preserve all three live states, all four terminal
states, the 32-record cap, lexical ties, optional finish times and idempotence.
The whole Super expose namespace validates the relocated tests and callers.

Compiled Cargo checks use Mojo 1.1.0 with `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`. The changed-test runner keeps default features.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/super_run_retention_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib rich::super_expose::run_policy::`
- `cargo test --locked -q -p prodex-app --lib super_expose::run:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib super_expose:: -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/crate-boundary-guard.mjs --self-test`
- `node scripts/ci/crate-boundary-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `npm run docs`
- `npm run test:changed`
- `npm run ci -- --jobs 1`
- `git diff --check`

An initial `mojo-rich`-only library test failed to link unrelated session
selector symbols selected by the runtime feature group. The production
`mojo-core` feature set is used for strict bridge validation instead; that
failed configuration is not counted as passing evidence. Native non-Linux,
live provider and full-workspace test execution remain outside this unit.
The 75% project target remains open.

Direct assertions, all three strict bridge tests, all ten run tests, all 106
Super expose tests and strict workspace Clippy pass. The first local CI run
found that moving the tests increased the unindented wildcard-import count;
the moved tests now use explicit imports and the unchanged wildcard guard
passes. Canonical production share is 36.31261863497566% (136,400 Mojo /
239,227 Rust production LOC).

The full local CI gate and changed-test runner pass. CI includes 685
runtime-launch tests, 43 auto-rotate integration tests and 282 Node tests;
the native Windows installer test is skipped on Linux. Feature-off zero-test
core harnesses in changed-test selection are not counted as compiled ABI
coverage. Ownership, authority, no-fallback, size, crate and secret-boundary
guards, final documentation lint and diff whitespace checks pass.
