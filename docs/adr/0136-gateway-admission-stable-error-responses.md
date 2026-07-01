# ADR 0136: Gateway admission stable error responses

## Status

Accepted

## Context

Gateway admission composes authorization, atomic reservation planning, provider
invocation validation, tenant-affinity checks, and telemetry span planning before
provider dispatch. Raw errors can reveal credential scopes, tenant identifiers,
reservation arithmetic details, provider credential references, or telemetry
implementation internals. Those details are useful for trusted diagnostics but
must not be returned through client-visible data-plane responses.

## Decision

Add `plan_gateway_admission_error_response` to `prodex-gateway-core`. It reuses
the stable authorization and provider invocation response planners where
possible, maps tenant/reservation admission failures to
`gateway_admission_rejected`, and maps telemetry planning failures to
`telemetry_unavailable`.

The response plan is HTTP-neutral and exposes only status, code, and generic
message fields. Raw errors remain available for trusted redacted diagnostics.

## Consequences

Gateway composition roots can reject pre-provider requests consistently without
leaking tenant IDs, credential scope internals, reservation overflow details,
provider secret references, or telemetry internals.
