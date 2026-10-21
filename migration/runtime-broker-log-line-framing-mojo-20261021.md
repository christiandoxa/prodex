# Runtime Broker Continuity Log Framing Migrated to Mojo (2026-10-21)

## Production ownership

The compiled runtime_broker_continuity.mojo owner now makes the
binary line-framing and bounded buffering decisions used while
reading persisted broker continuity logs. The versioned
prodex_runtime_broker_log_frame_plan_v1 ABI selects the first LF
boundary (or the entire partial slice), whether the line is complete,
an append/ignore/clear-oversized action, the next oversized state,
and whether a complete line is eligible for metric parsing.

The one-megabyte line cap is compiled Mojo policy. A line at the
cap is accepted; a byte beyond the cap suppresses that line through
its terminating LF. The next line is eligible again. Input remains
binary and need not be valid UTF-8.

Rust retains file reads and seeking, the original byte buffer,
cache management, and aggregation of parsed metrics, applying the
typed Mojo plan. The former Rust newline scan, size comparison,
append_runtime_broker_log_chunk helper, and metrics eligibility
branch have been deleted without a Rust fallback.

## Verification

All seven prodex-runtime-broker-log tests pass. Two new real-file
regressions cover partial tails, append completion, CRLF, oversized
line suppression and recovery. Three core ABI tests cover binary
input, exact size limits, invalid input parameters and untouched
results on rejected calls. The direct Mojo ABI test, anti-fallback
guard/self-test, Cargo formatting, and official source-share
non-regression check pass.

The audited production share increased from 36.11279186610626%
to 36.11452395546711% (133,744 Mojo / 236,589 Rust LOC).
The long-term 75% target remains unmet; no historical release floor
or accounting rule was weakened. Remote CI green was not required.
