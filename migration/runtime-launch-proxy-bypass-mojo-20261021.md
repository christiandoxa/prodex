# Runtime launch NO_PROXY normalization owned by Mojo (2026-10-21)

## Ownership and production activation

- Consumer: `crates/prodex-runtime-launch/src/child.rs`, used by the real
  Codex TUI and child-process plans for direct and governed providers.
- Compiled owner: `mojo/prodex_core/launch_proxy_env.mojo`, registered by
  `prodex-mojo-core/build.rs` under the real `mojo-runtime` feature.
- Typed ABI bridge: `crates/prodex-mojo-core/src/launch_config/proxy_bypass.rs`.
  Caller-owned UTF-8 views are read synchronously and checked original
  source indices, byte offsets and lengths are returned.
- Rust continues to own environment-variable lookup, lossless host/port URL
  parsing, native `OsString` conversion, and child-process side effects.
  Compiled Mojo alone splits comma-delimited candidates, trims Unicode
  whitespace, removes blanks, deduplicates with ASCII-only case folding,
  and preserves the first source spelling and input order.
- Deleted both Rust token-normalization and duplicate-detection helpers,
  including the prior per-token case-insensitive matching loop. There is
  no Rust policy fallback for invalid Mojo outputs.
- The bridge bounds the number of source views, total bytes and possible
  candidate tokens, and it validates all selected UTF-8 slices.

## Regression validation

- Direct Mojo versioned ABI fixture covers Unicode whitespace, duplicates,
  stable offsets/order, empty input, insufficient capacity, and malformed
  ABI inputs.
- Rust Mojo bridge unit cases verify ASCII folding over mixed Unicode,
  blank tokens, first occurrence and input-size rejection.
- Real runtime launch caller tests cover two inherited bypass keys, duplicate
  env precedence, IPv6 literals, default loopback, user-supplied and parsed
  proxy hosts, scoped child environments, and Unicode boundaries.
- The 75% project target remains pending; counting policy, historical
  non-regression baselines, and release-version metadata are untouched.

## Validation at checkpoint

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/launch_proxy_env_test.mojo`: PASS.
- `cargo test --locked -q -p prodex-runtime-launch`: 79 passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core proxy_bypass`:
  2 passed.
- `cargo test --locked -q -p prodex-app --lib runtime_launch::`:
  678 passed.
- Full locked workspace Clippy, all targets and features with `-D warnings`: PASS.
- Mojo anti-fallback and authority guards, size guard, production-share guard,
  Markdown lint, Cargo formatting and tracked-diff whitespace checks: PASS.
- Official share: 36.120514009120214% -> 36.13235527850587%
  (134,128 Mojo / 237,085 Rust production LOC). 75% is still unmet.
