# Runtime doctor report fields in Mojo (2026-10-22)

`runtime_doctor_fields_for_summary` now uses compiled fixed/optional row labels,
ordering, placeholders, count text, first-five diagnostic list clipping and
suffixes. Rust's row tables, field-ID match and compact-list formatter are
deleted without a fallback. Path display, summary-map acquisition, the separate
existing binding-state formatter and Serde field-pair materialization remain
in Rust. No dependency or counting rule changes.

The field-plan ABI and new renderer share pure optional-row and sync-source
kernels. The legacy public facade remains compatible. The new version-1 ABI
borrows fifteen text views, three diagnostic lists and unsigned counts/flags.
It returns bounded JSON field pairs through the existing escaped JSON writer.
An internal measurement pass rejects insufficient capacity before writing.
Wrong versions, invalid presence, alignment, UTF-8 and list metadata preserve
output and written-length sentinels. Empty strings remain distinct from missing
values; lists retain source order and duplicates; jobs precede profiles even
when their text is empty. Full-width unsigned counts retain decimal formatting.
The ownership manifest now points to the real renderer and its caller.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`.
Default CI/changed tests omit both and any archive override. Commands:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_doctor_report_render_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib rich::runtime_doctor_report_render::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib rich::runtime_doctor_report_`
- `cargo test --locked -q -p prodex-runtime-doctor --test report_fields -- --test-threads=1`
- `cargo test --locked -q -p prodex-runtime-doctor -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib doctor -- --test-threads=1`
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
- `npm run ci -- --jobs 1`
- `npm run test:changed`
- `npm run docs`
- `git diff --cached --check`
- `git diff --check`

Three caller regressions pass before/after migration, with direct Mojo, four
bridge tests, 70 doctor-crate and 27 app doctor tests. Strict workspace Clippy,
formatting, guards and full default CI pass; CI completes in 570.98 seconds
with 1,443 Rust tests. Changed tests pass in 80.57 seconds (70 Rust tests);
documentation, staged churn and diff checks pass. Evidence retains fixture
ownership/pointer corrections,
the obsolete manifest consumer marker and a Clippy fixture-borrow correction.
Canonical share is 36.4310533555615%
(137,454 Mojo / 239,845 Rust production LOC); the 75% target remains open.
Native non-Linux execution, live providers and exhaustive workspace/serialized
suites remain outside local coverage.
