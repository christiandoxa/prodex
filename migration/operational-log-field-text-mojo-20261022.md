# Operational log field text in Mojo (2026-10-22)

## Production ownership

Operational detail rendering now reuses compiled nonempty/control validation
and a compiled Unicode scalar prefix plan. Rust's `is_control` predicate and
both character scans are removed without a fallback. Map acquisition, existing
redaction, endpoint parsing, prefix copying and suffix materialization retain
their owners. The returned truncation flag alone selects the existing ellipsis.

`prodex_mojo_operational_log_text_span_v1` extends the existing compiled stream
owner. Its shared `rich_scalar_prefix_end` kernel replaces the audit truncator's
loop too. Audit still renders `...`; human log fields still render `…`. The
existing Super bounded-text ABI supplies control validation without another
classifier. No dependency, counting-rule or guard-threshold changes are made.

## Compatibility and boundary

The 192-character ordinary/endpoint limit and 32-character percent limit remain
Unicode scalar counts, including cuts between a base and combining mark. Empty
input, zero/full-width limits, leading/trailing whitespace and exact-boundary
inputs retain their results. C0/C1 controls are rejected before redaction;
NBSP, zero-width characters and BOM remain eligible. Plain sentinel suppression,
percent suffixes and URL privacy presentation are covered through real helpers.

The new ABI validates its version, UTF-8, a 64 MiB input bound, output presence
and alignment before writes. Production followed lines are already bounded to
2 MiB. Rust validates the returned UTF-8 boundary and truncation flag rather
than rescanning characters. Invalid ABI inputs leave output canaries unchanged.

## Verification

Focused Cargo checks use `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`. Default CI and changed tests omit both.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/operational_log_text_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib observability::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib audit_log_policy::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib rich::super_expose::`
- `cargo test --locked -q -p prodex-app --lib app_commands::log::log_stream::text_tests:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib app_commands::log:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-audit-log`
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
- `node scripts/ci/churn-hygiene.mjs --staged --check`
- `npm run docs`
- `npm run ci -- --jobs 1`
- `npm run test:changed`
- `git diff --cached --check`
- `git diff --check`

Direct Mojo, 12 observability, two audit and 32 Super bridge tests pass.
Three new caller regressions, all 99 log tests, 18 audit tests, workspace
Clippy and formatting pass. Full default CI passes in 480.17 seconds and
changed tests pass in 2.72 seconds. The Windows installer test is intentionally
skipped on Linux; strict positive bridge/caller counts supply Mojo execution
proof separately from feature-off harnesses. Documentation and final checks pass.
Canonical share is
36.380276339524094% (136,943 Mojo / 239,478 Rust production LOC); the checked
Rust span adapter increases the denominator slightly. The 75% goal stays open.
Native non-Linux execution, live-provider traffic and exhaustive workspace
tests are outside local coverage.

The no-fallback guard now scans the complete stream file: a mid-file test-only
I/O helper previously ended its production prefix too early. A regression
checks policy calls and restored scans after that helper. Test module
registrations remain together after production code.
