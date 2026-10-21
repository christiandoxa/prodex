# Codex configuration override removal in compiled Mojo (2026-10-21)

## Production ownership

The existing Codex-config owner now selects original arguments to remove via
`prodex_codex_config_removal_mask_v1`. Both the shared launch
`remove_first_codex_config_override_pair` and frontend
`remove_codex_config_override` use the same compiled mask through a minimal
Rust adapter. Their Rust scans, key splitting/trimming and inline-prefix
classification are deleted.

The owner reuses the existing Mojo assignment lexer. FirstPair removes the
first matching `-c` or `--config` pair. AllForms removes every matching pair or
inline `-cKEY=VALUE` / `--config=KEY=VALUE` argument. Both retain the original
behavior of scanning beyond `--`, preserve unrelated arguments byte-for-byte,
and keep malformed/unpaired flags. Paired values require original UTF-8;
inline keys use the platform's existing lossy view. Non-UTF-8 original arguments
are retained or removed as whole values, never rewritten.

Rust owns `OsString` acquisition and lossy conversion, applies the validated
mask to original values and reports whether the vector changed. Mojo owns all
matching and removal decisions; no Rust key parser or fallback remains.

## ABI and tests

The v1 ABI validates versions, mode tags, argument record alignment, original
encoding tags, UTF-8 display views and complete output capacity before writing.
It bounds 1,048,576 arguments, each field to 16 MiB and total argument text to
64 MiB, above ordinary OS argument limits. The host checks every mask byte.
The original first-pair API and frontend behavior remain distinct modes.

Direct Mojo and compiled Rust bridge tests cover duplicate keys, Unicode key
whitespace, inline/paired forms, separators, overlapping flags, unpaired flags,
original non-UTF-8 values, invalid views/tags/capacity and atomic rejection.
Application regressions verify both modes and byte-exact Unix OsString
preservation through the production shared helper.

## Validation commands

Use Mojo 1.1.0 and `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0` for direct
compiled Cargo checks:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/codex_config_removal_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib codex_config::`
- `cargo test --locked -q -p prodex-app --lib config_removal -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib app_commands::runtime_launch::tests -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib super_expose::run::tests -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/runtime-test-manifest-guard.mjs`
- `npm run docs`
- `npm run test:changed`
- `git diff --check`

The changed-test runner retains default feature selection. Live providers and
native non-Linux execution are outside this checkpoint. The 75% target remains
open until the official production-share report proves it.

Direct Mojo assertions, five strict bridge tests, two focused caller tests,
60 launch-flow tests and eight frontend tests pass. The runtime-test manifest
guard validates all 504 discovered runtime tests. Crate-boundary self-tests,
Mojo guards, the offline upstream baseline and all five replay fixtures pass.
The default changed-test runner also passes on the current implementation.
Strict workspace Clippy passes. The additional mixed valid/malformed-record
atomicity case passes when the five-test strict bridge suite is rerun.

After rebasing onto the session-store and MCP checkpoints, all strict bridge,
caller, launch, frontend, workspace Clippy and changed-test checks pass again.
Both production callers are explicitly included in the no-fallback guard scan.
Canonical production share is 36.28732388355108% (136,039 Mojo / 238,855 Rust
production LOC); the 75% target remains unmet.
