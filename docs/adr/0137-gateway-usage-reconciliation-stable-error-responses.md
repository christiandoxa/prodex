# ADR 0137: Gateway usage reconciliation stable error responses

## Status

Accepted

## Context

Gateway usage reconciliation runs after provider completion or interruption and
can fail because tenant-scoped inputs are inconsistent, actual usage exceeds the
reserved amount, or telemetry span planning fails. Raw errors can reveal tenant
identifiers, token/cost amounts, reservation arithmetic details, or telemetry
internals. Those details are useful for trusted diagnostics but should not be
returned through client-visible data-plane responses.

## Decision

Add `plan_gateway_usage_reconciliation_error_response` to
`prodex-gateway-core`. It maps tenant/reconciliation validation failures to
`usage_reconciliation_rejected`, and telemetry planning failures to
`telemetry_unavailable`.

The response plan is HTTP-neutral and exposes only status, code, and generic
message fields. Raw errors remain available for trusted redacted diagnostics.

## Consequences

Gateway composition roots can report post-provider reconciliation failures
consistently without leaking tenant IDs, usage amounts, reservation details, or
telemetry internals.
