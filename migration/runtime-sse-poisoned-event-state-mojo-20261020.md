# Runtime SSE Poisoned-Event Buffer Policy Migrated to Mojo (2026-10-20)

## Production ownership

The compiled sse_line.mojo owner now decides when an SSE event has data,
when a complete event should be emitted or discarded, and whether an
incoming data field should be appended, ignored, or cause a poisoned
event. The versioned prodex_runtime_sse_event_state_v1 ABI recognizes
the exact private NUL-prefixed invalid-UTF-8 marker and emits typed
decisions. This preserves the first-invalid-data poisoning of the
entire event, ignores later valid data until a blank-line/EOF
boundary, and allows the subsequent event to proceed.

Rust retains its streaming transport buffers, UTF-8 acquisition and
the original owned sentinel string, and acts on typed Mojo decisions.
The previous Rust sentinel classifier, invalid-event helper and
duplicate append/emit/payload selection policy branches were deleted
without a Rust semantic fallback.

## Verification

- All 424 prodex-runtime-proxy tests pass, including two new caller
  regressions testing invalid UTF-8 late in an event, suppression of
  subsequently valid data within that event, recovery after a blank
  boundary, EOF handling, and the payload parser gate.
- Two direct core ABI tests cover normal/empty/poisoned action
  matrices, exact sentinel equality, malformed tags, count/pointer
  validation and no output mutation on rejected calls.
- A direct Mojo ABI test covers all three operations and malformed
  inputs; strict no-fallback guard/self-test, Cargo formatting and
  canonical production-share non-regression checks pass.

Official production share rises from 35.88682132415225% to
35.88952865852326% (131,393 Mojo / 234,711 Rust production LOC).
The 75% goal remains unmet. No release floor, count definition or
metric baseline was altered; CI remote green was not a prerequisite.
