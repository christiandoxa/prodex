# ADR 0314: Gateway HTTP Control-Plane Route Error Responses

## Status

Accepted.

## Context

`plan_control_plane_route` can reject non-control-plane paths, unknown admin or
SCIM paths, and unsupported methods. Those raw errors contain trusted diagnostic
details such as operation names and HTTP methods. Adapters need a stable
client-visible envelope that does not expose route topology or operation
internals.

## Decision

`prodex-gateway-http` exposes `plan_gateway_control_plane_route_error_response`.
Unknown or non-control-plane routes map to `control_plane_route_invalid`.
Unsupported methods map to `control_plane_method_not_allowed`.

Both response messages are static and redacted. Raw route errors remain
available for trusted diagnostics.

## Consequences

HTTP adapters can fail closed with stable client-facing errors while preserving
diagnostic detail internally. Prefix-confusable paths, internal admin resource
names, operation names, and method values stay out of responses.
