# 0221: Application Data-Plane Wrong-Route Response

## Status

Accepted.

## Context

`prodex-application` rejects non-data-plane HTTP routes before gateway admission
so admin/control-plane paths cannot be treated as inference traffic. Earlier
application response planning left this wrong-route case without a client
response, which forced concrete adapters to invent their own mapping.

Different adapter mappings can leak internal route classification details or
turn a security boundary violation into inconsistent API behavior.

## Decision

Map `ApplicationDataPlaneError::WrongRoute` to a stable redacted HTTP response:

- status: `BadRequest`
- code: `route_not_available`
- message: `route is not available for this endpoint`

The response does not echo the rejected route, route kind, credential scope,
tenant ID, or authorization topology.

## Consequences

- Data-plane adapters can reject control-plane/admin routes consistently before
  admission or provider invocation.
- Client-visible errors remain stable and redacted.
- Admission and authorization failures continue to use their dedicated response
  planners.
