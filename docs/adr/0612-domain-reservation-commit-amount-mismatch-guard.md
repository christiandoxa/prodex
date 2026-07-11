# ADR 0612: Domain Reservation Commit Amount Mismatch Guard

## Status

Accepted.

## Context

Checked reservation commits verified tenant, call, and reservation IDs before
mutating budget state. They did not verify that the commit's reserved amount
matched the original reservation estimate.

## Decision

`validate_reservation_commit` now rejects commits whose `reserved` amount differs
from the reservation request estimate.

## Consequences

Commit retries cannot silently mutate accounting with a different reserved
amount for the same reservation ID. Mismatches fail behind the stable
`reservation_reserved_amount_mismatch` response code.
