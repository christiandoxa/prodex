# ADR 0591: Gateway HTTP Legacy Admin Route Boundary

## Status

Accepted

## Context

The legacy gateway admin adapter serves mounted routes under
`/prodex/gateway/...`, while the enterprise HTTP boundary primarily modeled
canonical `/admin/...` and `/scim/...` routes. That left compatibility admin
surfaces harder to route through the same explicit control-plane operation
contract before authorization.

## Decision

Canonicalize the legacy `/prodex/gateway/...` and
`/v1/prodex/gateway/...` mounts in `prodex-gateway-http` before classifying
control-plane routes. Add explicit read/update/delete operation variants for
legacy gateway admin, SCIM read, and virtual-key read/update/delete surfaces so
method, idempotency, and audit requirements are planned at the boundary. The
legacy mounted admin adapter calls the planner after successful admin
authentication, before route-specific handling, so method errors use the same
stable control-plane envelope as canonical routes.

## Consequences

- Legacy mounted admin routes can be routed through the same control-plane
  operation planner as canonical `/admin/...` routes.
- Read-only admin, SCIM, key, and ledger routes remain non-idempotency-key
  operations, while mutations still require idempotency and audit.
- The remaining legacy adapter still has hand-written route execution branches,
  but route classification and method rejection now pass through the shared
  HTTP boundary planner.
