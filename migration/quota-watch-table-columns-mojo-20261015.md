# Mojo quota-watch table column planning

The quota-watch TUI now gets its seven-column width plan from the compiled
`prodex_quota_watch_table_columns_v1` ABI in
`mojo/prodex_core/quota_watch_policy.mojo`. Mojo owns the width thresholds,
tie-breaking shrink loop, remaining-column floor, and tiny-terminal saturation.
Rust keeps terminal width acquisition, Unicode cell measurement, padding,
string construction, and Ratatui rendering.

Required-Mojo adapter and caller tests cover the threshold transitions,
zero-width input, and rendered row width. The former Rust allocation loop was
removed; there is no Rust fallback when the ABI fails.
