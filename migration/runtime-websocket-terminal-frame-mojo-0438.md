# Runtime WebSocket terminal-frame policy migration

`websocket_failure.mojo` now owns connection-limit detection and wrapped error
terminality through `prodex_runtime_websocket_terminal_frame_plan_v1`. The
kernel checks nested and top-level codes case-insensitively, preserves the
existing `status`-before-`status_code` precedence, and keeps committed frames
transparent. Rust retains JSON fact extraction, status-field presence handling,
wire forwarding, and retry effects.

Direct ABI and runtime-proxy caller regressions cover case folding, status
bounds and precedence, invalid input, precommit retry, and committed
transparency. Required-Mojo ABI tests (12) and WebSocket boundary tests (19)
pass, as do focused Clippy, formatting, ownership, authority, no-fallback, and
production-share guards. The broad report is 113,945 Mojo LOC of 338,165 total
(33.70%); the 75% project target remains in progress.
