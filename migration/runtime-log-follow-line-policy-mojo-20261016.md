# Runtime log follow line policy in Mojo

`prodex_live_log_follow_plan_v1` now owns followed-log complete-line size
classification, dropped-line counting, and pending-buffer gap decisions. Rust
keeps file reads, UTF-8 materialization, line splitting, gap-marker text, and
state mutation. Only bounded line lengths and buffer length cross the ABI, so
log payloads and secrets remain in Rust.

Required-Mojo ABI and `prodex-app` caller regressions cover exact and over-limit
lines and pending-buffer gaps.
