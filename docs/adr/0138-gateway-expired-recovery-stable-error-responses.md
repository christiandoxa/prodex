# ADR 0138: Gateway expired recovery stable error responses

## Status

Accepted

## Context

Gateway expired reservation recovery releases abandoned reservations and emits
billing ledger events. It can fail because tenant-scoped inputs are inconsistent,
a reservation is not yet expired, or telemetry span planning fails. Raw errors
can reveal tenant identifiers, usage amounts, reservation timing, or telemetry
internals. Those details are useful for trusted diagnostics but should not be
returned through client-visible or generic recovery-worker responses.

## Decision

Add `plan_gateway_expired_reservation_recovery_error_response` to
`prodex-gateway-core`. It maps tenant/recovery validation failures to
`expired_reservation_recovery_rejected`, and telemetry planning failures to
`telemetry_unavailable`.

The response plan is HTTP-neutral and exposes only status, code, and generic
message fields. Raw errors remain available for trusted redacted diagnostics.

## Consequences

Gateway composition roots and recovery workers can report expired-reservation
recovery failures consistently without leaking tenant IDs, usage amounts,
reservation timing, or telemetry internals.
