# Session-report filter precedence in compiled Mojo (2026-10-21)

- **Production consumer:** `prodex-session-store::session_report_matches_filter`
  used when enumerating and filtering available sessions.
- **Mojo owner:** `mojo/prodex_core/session_selector.mojo` exports the
  versioned `prodex_session_report_filter_v1` policy, called through the
  checked `prodex-mojo-core::json::session_report_filter_matches` bridge.
- One decision covers current-directory membership, subagent inclusion,
  exact case-sensitive profile matching, blank-query handling, and
  case-insensitive substring search across all six session fields.
  The original Rust decision chain was deleted without a Rust fallback.
- Rust retains platform path normalization and comparison as a host fact,
  and Unicode lowercase projection; compiled Mojo performs Unicode
  whitespace trimming and all matching and precedence decisions. Input
  pointers are borrowed for the call only; flags, UTF-8 validation, and
  boolean output tags are validated at the ABI boundary.
- **Offline regression:** direct Mojo ABI fixture covering Unicode whitespace,
  profile mismatch, subagent/dir exclusion, invalid ABI and invalid flags;
  Rust Mojo bridge regression for the combined gates; session-store
  integration tests for Unicode query, blank query, path, and profile.
- No change to session files, persisted state, release metadata, provider
  credentials, source-counting rules or the independent 75% target.

## Validation at checkpoint

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/session_report_filter_test.mojo`: PASS.
- `cargo test --locked -q -p prodex-session-store`: 62 tests passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core session_report_compiled_filter`: 1 test passed.
- `cargo test --locked -q -p prodex-app --lib app_commands::session::`: 6 tests passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`: PASS.
- Mojo no-fallback/authority guards, production-source share, size, cargo fmt,
  Markdown and change whitespace checks: PASS.
- Audited Mojo share: 36.139640888066104% -> 36.144144047212365%
  (134,372 Mojo / 237,395 Rust first-party production LOC). The 75% target
  remains unmet. No counting rules or release metadata changed.
