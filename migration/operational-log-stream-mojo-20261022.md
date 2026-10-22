# Operational log stream selection in Mojo (2026-10-22)

## Production ownership

The live operational timeline now uses `prodex_mojo_operational_log_stream_v1`.
Mojo owns payload/usage exclusion, exact profile-switch visibility, load-to-error
escalation, load field selection and endpoint presentation tags. Rust's matching
branches and field whitelist are deleted without a fallback. Rust retains the
existing log parser, borrowed map acquisition, endpoint URL parser, correlation,
redaction, rendering and collection effects.

The new operation calls the existing event classifier as a shared pure function.
Its public v1 facade keeps the same validation and results. Routine-load names
move into one pure helper used by both stream selection and aggregation; the
old literal matcher is removed. No dependency or counting rule changes.

The ABI validates version, UTF-8 views, up to 1,048,576 fields, a 64 MiB aggregate
decoded byte bound and output capacity before writes. Existing 65,536-byte
event/selected classifier-field bounds stay intact. Rust checks returned source,
load flags and every field tag; field indices derive from the validated mask.
Observability is an independently compilable feature, so its private view record
does not depend on the separately gated rich/JSON Rust bridges.

## Compatibility

Only exact `switched=true` reveals a `profile_commit`. Payload and usage records
stay outside operational insights. Nonroutine overloads remain errors rather
than load observations; the three routine events keep raw JSON observations
and existing human-view aggregation. All eleven retained field names, empty
values, input order and endpoint sanitization are preserved. Irrelevant fields
cannot enter load observations. Duplicate bridge keys select the first value;
the production parser supplies a unique-keyed `BTreeMap`.

## Verification

Strict Cargo commands use `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`; default changed tests omit both.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/operational_log_stream_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib observability::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --test log_load`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-observability --lib observability::`
- `cargo test --locked -q -p prodex-app --lib compiled_operational_selection -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib app_commands::log:: -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/super-wildcard-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `node scripts/ci/crate-boundary-guard.mjs --self-test`
- `node scripts/ci/crate-boundary-guard.mjs`
- `node scripts/ci/production-boundary-guard.mjs --self-test`
- `node scripts/ci/production-boundary-guard.mjs`
- `node scripts/ci/churn-hygiene.mjs --staged --check`
- `npm run docs`
- `npm run test:changed`
- `git diff --cached --check`
- `git diff --check`

Direct Mojo, ten observability bridge tests in each feature configuration,
four load aggregation tests, three new production-caller regressions and all
96 log tests pass. Workspace Clippy, formatting, boundary and ownership guards
pass. Documentation and changed-test selection pass (3.66 seconds); feature-off
zero-test harnesses do not replace the strict positive counts. Early compiler/test
failures are preserved in local evidence: explicit view copies/conversions and
supported array syntax replaced invalid constructions, and raw ABI tests now
use heap-backed outputs rather than stack arrays read after address erasure.
The wildcard guard also caught a new glob import, which was replaced without
changing its cap. An incorrect caller filter ran zero tests; the runner rejected
that evidence and the corrected filter passes three tests. Canonical share is
36.38097971069944% (136,922 Mojo / 239,434
Rust production LOC). The 75% goal remains open. Native non-Linux execution,
live-provider traffic and exhaustive workspace tests remain outside coverage.
Full default CI passed at the preceding message checkpoint and is not repeated
for this focused unit.
