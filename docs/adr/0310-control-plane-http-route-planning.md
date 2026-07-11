# ADR 0310: Control-Plane HTTP Route Planning

## Status

Accepted.

## Context

Enterprise gateway adapters must not treat every `/admin` or `/scim` request as
a generic admin action. The HTTP boundary needs to bind method and path to an
explicit control-plane operation before authorization, idempotency, audit, or
storage mutation are adapted.

## Decision

`prodex-gateway-http` exposes `plan_control_plane_route`, a framework-neutral
planner for `/admin`, `/v1/admin`, `/scim`, and `/v1/scim` requests. The plan
resolves each supported method/path pair to a `GatewayControlPlaneOperation`
and carries whether the operation requires idempotency and immutable audit.

Unknown control-plane paths, data-plane paths, and unsupported methods fail
closed before downstream adapters can call generic control-plane logic.

## Consequences

HTTP adapters can wire legacy admin and SCIM routes incrementally while keeping
operation identity explicit. The planner deliberately stays independent from
`prodex-control-plane` so the HTTP boundary does not create a control-plane
crate dependency; concrete composition code can translate the operation enum at
the edge.
