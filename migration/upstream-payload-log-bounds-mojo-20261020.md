# Upstream Payload Log Byte Limits Migrated to Mojo (2026-10-20)

## Compiled production policy

The production log_semantics.mojo owner now determines the cumulative
byte budget for JSON string values and exactly where to cut an
oversized logged payload. Two versioned ABIs are called directly
from the runtime proxy's live upstream-payload log path:

- prodex_mojo_upstream_payload_string_budget_v1 computes omission
  flags for ordered Serde string values, retaining the remaining
  budget when one value exceeds it; later shorter values may still
  fit. The host partitions unusually large borrowed sets into
  bounded 1,048,576-item batches and carries the Mojo-owned remaining
  budget to the next batch.
- prodex_mojo_upstream_payload_truncate_v1 enforces the source byte
  cap. A UTF-8-valid prefix stays intact; an invalid capped prefix
  falls back to the last ASCII byte, preserving the preexisting
  conservative semantics exactly, including incomplete Unicode
  scalars at the truncation boundary.

Rust retains Serde parsing, generic borrowed traversal to collect
string-value references, application of Mojo-selected omission flags,
the existing marker text, secret redaction, binary base64 encoding and
event logging. No Rust byte-budget branch or UTF-8 truncation policy
fallback remains. Redaction is still applied **after** limiting each
sensitive JSON value and before any payload is logged.

## Validation

- Five prodex-app upstream-payload-log tests pass, including old
  secret-redaction, JSON size and UTF-8 behavior and new tests for
  nested maps/arrays, skipped long strings, 64-KiB caps, malformed
  byte strings, incomplete multibyte characters and ASCII fallback.
- Three direct core ABI tests pass for exact budget order, empty and
  oversized values, Unicode clipping, malformed version/count/capacity
  and unchanged output on rejected calls.
- Direct Mojo ABI tests, no-fallback guard/self-test, Cargo formatting
  and official production-share non-regression checks pass.

Canonical production share increased from 35.88952865852326% to
35.89055457119222% (131,461 Mojo / 234,822 Rust production LOC).
The 75% project target is still unmet. No accounting definitions,
release floors or non-regression gates were weakened. Remote CI
green was not a checkpoint prerequisite.
