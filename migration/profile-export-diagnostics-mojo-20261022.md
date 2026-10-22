# Profile export schema diagnostics in Mojo (2026-10-22)

Profile-export schema errors now use a compiled diagnostic renderer for all
seventeen fixed/numeric messages. Rust's template match and optional-number
defaults are removed. Rust retains crypto, encodings, Serde, existing validation
calls and the original Mojo transport-error context. No credential, password,
salt, nonce or ciphertext content crosses the new diagnostic ABI.

The version-1 ABI accepts only issue tags and optional unsigned 32-bit metadata,
with explicit absence sentinels, and returns bounded caller-owned text. Mojo
owns absent-value zero/default handling, ciphertext-length message aliases,
expected-kind-version fallback and decimal formatting. It reuses PBKDF2 bounds
from the compiled policy owner; validator and diagnostics share supported-version
constants. The diagnostic module keeps the large policy file focused. Invalid
tags, numeric bounds, alignment and insufficient capacity preserve output and
length sentinels. Existing schema APIs, secret handling and crypto policy remain
compatible. No dependency, Rust fallback or counting-rule change is added.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`.
Default changed tests omit both and any archive override. Commands:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/profile_export_diagnostics_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib profile_export::diagnostics::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib profile_export::`
- `cargo test --locked -q -p prodex-profile-export --lib envelope::diagnostic_tests:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-profile-export -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib profile_export_ -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib profile_import_ -- --test-threads=1`
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

Two caller regressions pass before/after migration, with direct Mojo, 16 bridge
tests, all 62 export-crate tests, 12 app export and 25 app import tests. Strict
workspace Clippy, formatting, ownership, authority, no-fallback, size and secret
guards pass. Changed tests pass in 74.26 seconds with 62 Rust tests;
documentation, staged churn and diff checks pass.
An existing test module needed explicit PBKDF2 constant imports after the parent
formatter stopped using them; that initial compile failure is retained in
evidence, together with an overly specific app test filter rejected for selecting
zero tests before its corrected positive runs. Canonical share is
36.460421140144405% (137,707 Mojo / 239,982 Rust
production LOC). The 75% goal remains open. The earlier report checkpoint passed
full default CI; this diagnostic change uses focused export/import suites and
strict workspace Clippy. Native non-Linux execution, live providers and exhaustive
workspace/serialized suites remain outside local coverage.
