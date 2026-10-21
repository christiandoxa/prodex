# Super run event retention and cursor pages in Mojo (2026-10-22)

## Production ownership

The existing Super run policy owner now plans event insertion and cursor pages.
Mojo owns the fixed 256-event cap, oldest-event eviction, saturating unsigned
sequence increment, strict cursor comparison, page cap and lost-event marker.
The Rust filter/take loop, cursor arithmetic, sequence arithmetic, pop loop and
cap constant are deleted without a fallback. All lifecycle/output event
callers and the live protocol page path use the compiled decisions.

Rust retains the deque, strings, redaction, UTF-8 text acquisition, locks and
process effects. It applies checked indices and mutation counts. Page errors
propagate through the existing optional page boundary; insertion errors leave
the deque and sequence unchanged. Storage is not cleared or recomputed after
an ABI failure.

Both v1 exports validate version, counts and output alignment before writes.
The page export also bounds count/byte size, checks input/index alignment and
requires caller-owned index capacity. Full-width unsigned cursors, duplicate
or nonmonotonic sequences and an empty deque retain their previous behavior.
The insertion plan handles an oversized deque in one bounded drain. Index and
tag validation remains in the Rust trust boundary.

## Verification

Direct assertions cover the fixed cap, requested-limit saturation, arbitrary
deque order, duplicate maximum sequences, empty pages, cursor gaps and atomic
ABI rejection. Compiled bridge checks exercise the same edge classes; caller
regressions retain real deque contents, unchanged next cursors and the full
unsigned sequence range. Existing event pagination and post-truncation stdout
tests stay active. The existing terminal-retention tests run with the expanded
bridge and shared status decoder.

Strict Cargo checks use Mojo 1.1.0 with `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`. Default CI and changed-test feature selection omit
those variables.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/super_run_event_test.mojo`
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

Native non-Linux, live provider and full-workspace test execution are outside
this offline unit. The 75% project target remains open.

Direct Mojo assertions, all six strict run-policy bridge tests, all 12 run
tests, all 108 Super expose tests and strict workspace Clippy pass. Authority,
ownership, no-fallback self-test, size, crate-boundary self-test/check,
secret-boundary, documentation and whitespace checks pass. Canonical share is
36.31104286805347% (136,450 Mojo / 239,331 Rust production LOC). The additional
checked Rust ABI adapters slightly reduce raw source share; the counting rules
and guards are unchanged.

The full local CI gate and changed-test runner pass. CI includes 685
runtime-launch tests, 43 auto-rotate tests and 282 passing Node tests; its
Windows installer test is skipped on Linux. Default changed-test selection
uses feature-off, zero-test core harnesses, so it is not counted as compiled
ABI coverage. The six strict bridge tests provide that evidence.
