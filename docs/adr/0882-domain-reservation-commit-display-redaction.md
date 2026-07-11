# ADR 0882: Domain reservation commit display redaction

## Status

Accepted.

## Context

ADR 0880 made reservation commit mismatches use the stable response-planner
message in raw `Display` output. Other `ReservationCommitError` variants still
named zero-actual, actual-exceeds-reserved, reserved-balance-underflow, and
committed-usage-overflow classes.

## Decision

Render every `ReservationCommitError` variant as `reservation commit rejected`.
Keep response planners and typed variants unchanged so callers can still return
precise low-cardinality codes and classify failures for audit or metrics.

Regression coverage pins representative exact display strings and keeps the
existing checks that usage amounts are absent from display output.

## Consequences

Raw accounting commit display output no longer reveals usage validation or
balance classifications. Trusted diagnostics should match typed variants when
they need the exact reason.
