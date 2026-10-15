# Runtime WebSocket precommit hold state in Mojo

`websocket_response_tracking.mojo` now owns bounded precommit hold accumulation, hard-affinity rejection, and lookahead-versus-event promotion through `prodex_websocket_precommit_hold_plan_v1`. Rust still acquires event facts, buffers text, logs the existing markers, and applies the validated plan without moving socket or stream effects across the ABI. Compiled ABI cases and the existing WebSocket hold regressions cover promotion precedence, the byte limit, and fail-closed forwarding.
