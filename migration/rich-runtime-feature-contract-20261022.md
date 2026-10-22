# Rich/runtime feature dependency repair (2026-10-22)

`mojo-rich` now declares its existing `mojo-runtime` dependency. The rich JSON
API already references runtime/session exports, but selecting rich and routing
without runtime omitted their Mojo link inputs. Cargo now selects those inputs
through the feature graph. The prior undefined-symbol failure is retained as
regression evidence; the identical explicit rich/routing test command now links
and passes. All 968 tests in that feature configuration pass as well.

This is a build-contract repair, not a new semantic migration. No Rust or Mojo
algorithm, ABI, fallback, dependency package or counting rule changes. The full
core and workspace all-features configurations already enable runtime. Version
synchronization passes without metadata changes, and the lockfiles are unchanged.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`.
Default metadata preflight and changed tests omit both and any archive override.
Validation commands:

- `cargo test --locked -q -p prodex-mojo-core --features mojo-rich,mojo-routing --lib log::active_paths::tests::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-rich,mojo-routing --lib -- --test-threads=1`
- `cargo test --locked -q -p prodex-cli -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `npm run npm:sync-version`
- `node scripts/ci/crate-boundary-guard.mjs --self-test`
- `node scripts/ci/crate-boundary-guard.mjs`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `node scripts/ci/churn-hygiene.mjs --staged --check`
- `npm run ci -- --jobs 1 --no-tests`
- `npm run test:changed`
- `npm run docs`
- `git diff --cached --check`
- `git diff --check`

The previously failing three bridge tests and all 968 feature-library tests pass,
with strict workspace Clippy, formatting, version synchronization and both crate
boundary checks. Metadata/Clippy preflight passes in 139.56 seconds with tests
explicitly disabled; it is not reported as full CI. The preceding active-log
checkpoint passed full default CI with 1,443 Rust tests. All 153 CLI tests pass.
Changed tests pass in 0.71 seconds with zero selected Rust tests; the positive
feature/CLI runs supply coverage. Documentation, final guards and diff checks
complete validation.

Canonical share remains 36.481147777398505% (137,867 Mojo / 240,046 Rust
production LOC). The 75% objective stays open. Native non-Linux execution, live
providers and exhaustive workspace/all-serialized suites are outside this local
validation. No new full default CI run is claimed for this manifest-only repair.
