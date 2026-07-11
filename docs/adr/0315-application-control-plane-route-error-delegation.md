# ADR 0315: Application Control-Plane Route Error Delegation

## Status

Accepted.

## Context

`prodex-gateway-http` owns the route-level response contract for control-plane
HTTP planner failures. `prodex-application` consumes those route plans when
validating admin idempotency. If application code remaps every route failure to
a generic bad request, unsupported methods lose their stable method-not-allowed
semantics and adapters get inconsistent responses.

## Decision

Application control-plane idempotency errors now delegate `HttpRoute` failures
to `plan_gateway_control_plane_route_error_response`. The application response
status preserves `MethodNotAllowed` while reusing the gateway HTTP code and
message.

`ci:application-boundary-guard` checks for this delegation and for the
application-level method-not-allowed status.

## Consequences

Composition roots have one client-visible route error contract for
control-plane HTTP idempotency. Operation names, methods, fingerprints, and
tenant identifiers remain out of responses while method-denied cases stay
distinguishable from unknown routes.
