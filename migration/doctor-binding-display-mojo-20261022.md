# Runtime doctor binding display in Mojo (2026-10-22)

`runtime_doctor_fields_for_summary` now obtains binding text from the compiled
binding owner. Rust's `binding_source_text`, `binding_state_text`, templates,
absent-name fallbacks and missing-profile count sum are deleted. The host borrows
numeric source metadata and the first profile from the existing compiled source
ranking. Mojo formats all counts, names and four source segments. The published
report-render API still accepts the completed binding text unchanged.

The version-1 ABI uses four fixed source records with unsigned counts and
borrowed names. Rust/Mojo layout assertions cover the 88-byte source and
392-byte input. Mojo validates flags, UTF-8, alignment and pointers before
measuring output; insufficient capacity leaves output/length untouched. Sources
retain state/runtime/journal/merged order and use the original bounded summary
counts. Empty names remain present; absent names use a dash. Full-width counts,
Unicode and embedded newlines retain their exact text. No dependency, fallback,
duplicate production formatter or counting-rule change is added.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`.
Default changed tests omit both and any archive override. Validation commands:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_doctor_binding_display_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib rich::runtime_doctor_binding_display::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib rich::runtime_doctor_`
- `cargo test --locked -q -p prodex-runtime-doctor --test binding_display -- --test-threads=1`
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
- `npm run test:changed`
- `npm run docs`
- `git diff --cached --check`
- `git diff --check`

Two caller regressions pass before/after migration, with direct Mojo, 55 doctor
bridge tests, 72 doctor-crate and 27 app doctor tests. Strict workspace Clippy,
formatting, ownership, authority, no-fallback, size and secret guards pass.
Changed tests pass in 88.11 seconds with 72 Rust tests; documentation, staged
churn and diff checks pass.
Canonical share is 36.438917910843145%
(137,554 Mojo / 239,938 Rust production LOC); the 75% goal remains open.
The preceding checkpoint passed full default CI; this narrower presentation
change uses focused caller/crate coverage and strict workspace Clippy. Native
non-Linux execution, live providers and exhaustive workspace/serialized suites
remain outside local coverage.
