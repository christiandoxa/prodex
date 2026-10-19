# Compiled log TUI key transitions — 2026-10-19

The interactive log TUI keyboard policy is now a required Mojo-owned state
transition in the existing production log_semantics.mojo module. The versioned
prodex_mojo_log_tui_key_plan_v1 ABI receives the terminal-decoded key kind,
Unicode character scalar, control modifier, search-edit mode, and current
scroll offset plus host word-sized maximum. It emits the next edit mode,
search mutation (unchanged/clear/pop/push), quit decision, and bounded scroll
offset.

This removes the Rust special-key decision tree, scroll/page increments,
control-shortcut precedence, and search-edit branching from the live
LogTuiState::apply_key production path. Rust now only normalizes crossterm
key shapes, retains UTF-8 text, and applies the validated action. No Rust
fallback or second keyboard state machine remains.

Direct compiled ABI tests cover all key families, control combinations,
Unicode scalars, editing modes, saturated scrolling, and malformed inputs
without changing the output buffer. The app regression validates actual
search mutations and Unicode event flow. The no-fallback guard requires the
compiled symbol, direct tests, caller coverage, and forbids restoring the
former Rust scroll and key-branch policy.

Validation:

    mojo build --emit object -I mojo/prodex_core -o /tmp/prodex-log-tui-key-policy-20261019.o mojo/prodex_core/log_semantics.mojo
    PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -p prodex-mojo-core --features mojo-core --lib log::tui::tests
    PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -p prodex-app --lib app_commands::log_tui::tests
    node scripts/ci/mojo-no-fallback-guard.mjs --self-test
    node scripts/ci/mojo-production-share.mjs --check --json

The project-wide 75% Mojo objective remains open; this checkpoint claims
only the actual production migration measured by the repository script.
