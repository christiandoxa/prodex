# ADR 0716: Redact Reservation Commit Error Details

## Status

Accepted

## Context

Reservation commits are part of tenant-owned accounting. The domain commit
errors keep structured tenant, call, reservation, and usage amount fields so
callers can classify failures. Their `Display` strings still echoed those
values, which is too detailed for regulated API and log boundaries.

## Decision

Keep the structured error variants, but make reservation commit mismatch and
commit failure `Display` strings generic. Stable response planners continue to
return fixed machine-readable accounting envelopes.

## Consequences

- Tenant IDs, call IDs, reservation IDs, and usage amounts are not echoed through
  reservation commit error strings.
- Internal code can still match variants and inspect structured fields where
  trusted diagnostics need them.
- Accounting error handling stays aligned with stable redacted response
  requirements.
