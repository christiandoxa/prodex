# Optional-tool probe-version token selection in Mojo (2026-10-21)

## Production ownership

- **Consumer:** `prodex-optional-tools` resolves executable versions before
  accepting installed tools and their minimum-version contracts.
- **Compiled owner:** `mojo/prodex_core/optional_tools_policy.mojo` exports
  `prodex_optional_tool_probe_token_v1` through a checked, caller-owned
  `crates/prodex-mojo-core/src/optional_tools_policy/probe_version.rs` adapter.
- Mojo exclusively owns Unicode-whitespace token boundaries, stripping
  leading/trailing non-ASCII-alphanumeric-or-semver-punctuation characters,
  stripping repeated lowercase `v` prefixes, and lexical probe-token order.
  Rust no longer has a duplicate scanner or lexical fallback.
- Rust retains `semver::Version::parse` as the mature dependency boundary,
  the minimum-supported-version comparison, and process/stdio I/O.
  Each call returns a bounded UTF-8 byte range and the next token cursor.
  The bridge verifies exact offsets, monotonic forward progress, Unicode
  boundaries, versioned ABI status, and input-size limits.
- Original optional-tool version scanning was extracted from the near-limit
  parent source into a focused module with deterministic regression coverage.
  No new dependency and no release-version change.

## Offline validation

- Direct Mojo ABI test checks UTF-8 offsets, Unicode whitespace, repeated
  leading `v`, end-of-input, invalid cursor and ABI version.
- Rust bridge tests check candidate order, source ranges, nonboundary
  cursor rejection, and size caps.
- Optional-tool caller tests cover prerelease values, invalid diagnostic
  text, a lower minimum version, and acceptance of a newer version.
- `cargo test --locked -q -p prodex-optional-tools`: 66 passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core compiled_probe_tokens`:
  2 passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`: PASS.
- Mojo no-fallback, authority and size guards: PASS.
- Canonical share: 36.144144047212365% -> 36.15095049877659%
  (134,449 Mojo / 237,461 Rust first-party production LOC).
  The separate 75% objective remains unmet.
