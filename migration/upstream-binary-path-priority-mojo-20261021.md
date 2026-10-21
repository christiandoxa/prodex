# Upstream binary log display path priority in compiled Mojo (2026-10-21)

## Production ownership

- **Consumer:** `crates/prodex-app/src/app_commands/log_upstream_payload.rs`,
  used when rendering binary upstream logs to the user.
- **Compiled owner:** `mojo/prodex_core/log_semantics.mojo` exports
  `prodex_mojo_upstream_binary_path_plan_v1`, reached through a checked
  `prodex-mojo-core/src/log/upstream_binary_path.rs` bridge.
- Mojo selects the *first present* of the seven known log fields:
  `path`, `file_path`, `local_path`, `absolute_path`, `image_path`,
  `attachment_path`, and `uri`. It returns the Unicode-whitespace-trimmed
  UTF-8 byte span. An empty value on a higher-priority key prevents
  falling through to later keys, preserving the old `find_map` semantics.
  Rust's priority `find_map` and `.trim().filter` decision were removed;
  no Rust fallback makes an alternate selection.
- Rust owns map lookups and final borrowed string materialization for
  terminal output, without contacting external URLs or opening the path.
  The Mojo ABI validates all seven input records before writing output,
  distinguishes absent from present-empty entries, bounds strings to
  1 MiB, and returns an optional index and span. The Rust adapter
  validates UTF-8 boundaries and nonempty output without policy fallback.
- No filesystem mutations, sensitive provider traffic, new dependencies,
  counting exceptions, or release version changes.

## Offline validation

- Direct compiled Mojo test covers path precedence, empty high-priority
  values, ASCII trimming, absence, bad ABI/count and oversized later fields.
- Rust bridge tests cover Unicode whitespace, first-present behavior,
  last-priority URLs and per-field input size bounds.
- Production app caller test covers metadata map presence, Unicode
  values and prevention of lower-priority fallback after a blank path.
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/log_upstream_binary_path_test.mojo`: PASS.
- `cargo test --locked -q -p prodex-app --lib binary_path`: 1 passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core compiled_upstream_path_policy`:
  bridge regression passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`: PASS.
- Cargo formatting, source-share, Mojo no-fallback and authority, size
  and docs guards: PASS.
- Canonical share increased from 36.17922036833963% to 36.18283864120034%
  (134,996 Mojo / 238,098 Rust production LOC).
  The separate 75% target remains unmet.
