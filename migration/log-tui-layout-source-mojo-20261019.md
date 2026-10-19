# Log TUI layout and source-tone migration — 2026-10-19

## Scope and authority

The production log_semantics.mojo translation unit now owns three deterministic
log-rendering policies through versioned, required ABI functions:

- prodex_mojo_log_tui_window_v1 selects a scrollable line window, using
  checked unsigned arithmetic for saturated bottom-scroll behavior.
- prodex_mojo_log_tui_snapshot_height_v1 chooses the minimum and maximum
  snapshot heights, caps oversized host line measurements before adding row
  overhead, and handles absent/present snapshot sections.
- prodex_mojo_log_tui_source_tone_v1 classifies exact event sources and
  the tool-call prefix into terminal palette identifiers.

Rust's retained responsibility is measuring actual ratatui content,
materializing the selected line range, and mapping validated tone IDs to
ratatui::style::Color. The former Rust viewport arithmetic, snapshot
height formula and source classifier were removed; there is no Rust
semantic fallback or duplicated production decision.

## Validation

Run from the workspace root:

    mojo build --emit object -I mojo/prodex_core -o /tmp/prodex-log-tui-policy-20261019.o mojo/prodex_core/log_semantics.mojo
    PRODEX_MOJO_REQUIRED=1 cargo test --locked -p prodex-mojo-core --features mojo-core --lib log::tui::tests
    PRODEX_MOJO_REQUIRED=1 cargo test --locked -p prodex-app --lib log_command_tui::render::tests
    PRODEX_MOJO_REQUIRED=1 cargo test --locked -p prodex-app --lib app_commands::log_tui::tests
    node scripts/ci/mojo-no-fallback-guard.mjs --self-test
    node scripts/ci/mojo-production-share.mjs --check --json

The ABI tests cover empty, zero-height and overflow-sized viewports,
snapshot section presence and row clamps, exact case-sensitivity,
and Unicode tool-call prefixes. Real app-level tests protect rendering,
source palette and scrolling. The no-fallback guard detects restored
production Rust decisions and requires the compiled ABI entrypoints.

Only deterministic log TUI policy is affected; event ingestion, snapshot
content generation and terminal I/O remain Rust host responsibilities.
