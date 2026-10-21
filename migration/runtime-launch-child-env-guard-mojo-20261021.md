# Child environment hardening-key classification in Mojo (2026-10-21)

## Production ownership

- Runtime consumer: `crates/prodex-runtime-launch/src/child.rs` builds the
  inherited-environment removal list for all child launch plans.
- Compiled owner: `mojo/prodex_core/launch_child_env.mojo`, enabled under the
  existing runtime feature and linked through
  `crates/prodex-mojo-core/src/launch_config/child_env.rs`.
- Mojo exclusively classifies exact inherited key names: sandbox variables
  starting with `CODEX_SANDBOX`, dynamic-loader names starting with `DYLD_`,
  and the exact unsafe-child-env opt-out key. The Rust prefix matching and
  bypass detection are deleted, with no fallback decision path.
- Rust continues to own raw `OsString` capture, ordered removal sets,
  the existing fixed platform-loader key names, launch side effects, and
  explicit opt-out application. Non-UTF-8 key names remain opaque to Mojo,
  matching the previous Rust `OsStr::to_str` guard.
- All keys in the borrowed C ABI use a validity discriminator, and all tags
  are checked against the typed Rust bridge before any removal-plan mutation.
  The compiled function validates the full input batch before output writes.
- Neither child environment values nor credentials cross the ABI or enter
  diagnostics. Unknown ABI responses fail closed, not by recomputing a Rust
  policy.

## Regression scope

- Direct Mojo tests: classified prefixes, exact opt-out, non-UTF-8
  discriminator, empty batch, invalid ABI, invalid capacity and tagged input.
- Rust bridge tests: exact/uppercase-sensitive tags, opaque keys and size caps.
- Runtime caller tests: retained defaults, case-sensitive matching, empty
  opt-out value, and Linux non-UTF-8 variable names.
- No fallback Rust implementation, release change, or counting exemption.

## Validation at checkpoint

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/launch_child_env_test.mojo`: PASS.
- `cargo test --locked -q -p prodex-runtime-launch`: 81 passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core child_env`:
  2 passed.
- `cargo test --locked -q -p prodex-app --lib runtime_launch::`:
  678 passed.
- Locked full workspace Clippy with `-D warnings`: PASS; Mojo ABI built.
- Mojo no-fallback, authority and size guards, Markdown lint, Cargo format
  and production-source counting non-regression checks: PASS.
- Official source share: 36.13235527850587% -> 36.13600379134668%
  (134,199 Mojo / 237,173 Rust production LOC); 75% is still pending.
