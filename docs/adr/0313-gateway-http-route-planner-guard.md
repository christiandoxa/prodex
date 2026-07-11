# ADR 0313: Gateway HTTP Route Planner Guard

## Status

Accepted.

## Context

The HTTP boundary now classifies admin and SCIM routes into explicit
control-plane operations. This is security-sensitive because raw prefix matching
or missing method checks could let prefix-confusable paths or unsupported
methods reach control-plane authorization with the wrong operation.

## Decision

`ci:gateway-http-boundary-guard` now checks `prodex-gateway-http` source for the
control-plane route operation enum, `plan_control_plane_route`, fail-closed route
errors, segment-boundary admin and SCIM mount matching, versioned route support,
method-specific operation checks, and idempotency/audit flags in the route plan.

The guard self-test rejects sources that remove the method gate or reintroduce
raw `starts_with` matching for admin or SCIM mounts.

## Consequences

Route planner regressions are caught by both Rust tests and the enterprise
HTTP boundary guard. Future route additions can remain incremental, but they
must preserve fail-closed, method-aware, segment-boundary behavior.
