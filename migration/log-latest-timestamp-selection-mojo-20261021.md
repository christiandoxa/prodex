# Latest log event timestamp selection owned by compiled Mojo (2026-10-21)

## Production ownership

- **Consumers:** seven newest-event comparisons in
  `crates/prodex-app/src/app_commands/log_transcript.rs`,
  `log_upstream.rs`, and `log_command_tui.rs` (runtime transcript,
  upstream payload, token usage and live snapshots).
- **Compiled owner:** `mojo/prodex_core/log_semantics.mojo` exports
  `prodex_mojo_log_latest_timestamp_v1` through
  `prodex-mojo-core/src/log/latest_timestamp.rs`.
- The canonical decision is to select the incoming event when no incumbent
  exists, or when the incoming timestamp is lexically greater *or equal*.
  This preserves last-seen precedence on equal timestamps, plus the
  byte-wise UTF-8 `str::Ord` comparison used by the original Rust code.
  Seven duplicated Rust `event.timestamp >= current.timestamp` conditions
  were deleted; Mojo is the sole decision owner with no Rust fallback.
- Rust retains event acquisition from logs, file IO, timestamp extraction,
  optional event storage and rendering. A shared narrow application adapter
  lives in `app_commands/log_format.rs` alongside timestamp formatting,
  avoiding the repo's near-limit `log_tui.rs` file. The policy requires
  no new dependencies, raw filesystem access or release metadata changes.
- Inputs are borrowed validated UTF-8 timestamp views; length, presence,
  pointer alignment and boolean output are checked across the versioned ABI.

## Offline validation

- Direct Mojo ABI fixture verifies later/earlier/equal/absent timestamps,
  invalid presence tag, invalid ABI and invalid oversized input.
- Rust bridge tests validate Unicode lexical order, equal events and absent
  incumbents. App log suites exercise the shared wrapper and seven callers.
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/log_latest_timestamp_test.mojo`: PASS.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core compiled_newest_timestamp`:
  2 passed.
- `cargo test --locked -q -p prodex-app --lib app_commands::log_`: 53 passed.
- Full workspace Clippy with `-D warnings`, Mojo no-fallback and authority
  guards, 32/32 near-limit size guard, docs and formatting checks: PASS.
- Official share: 36.17372536702826% -> 36.17480355532655%
  (134,796 Mojo / 237,828 Rust production LOC). The 75% target is unmet.
