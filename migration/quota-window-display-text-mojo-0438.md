# Quota window display text migrated to Mojo

`prodex_quota_window_text_v1` now composes the quota window pairs, reset
summary, full and compact status, and detail strings. Rust retains window
lookup plus Chrono/local-timezone conversion and passes borrowed reset text
through the typed ABI. Missing windows, unknown usage, signed numeric bounds,
and UTF-8 bytes keep their existing output.

Required-Mojo ABI and `prodex-quota` caller regressions cover the boundary.
