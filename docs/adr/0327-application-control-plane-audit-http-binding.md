# ADR 0327: Application Control-Plane Audit HTTP Binding

## Status

Accepted.

## Context

Control-plane operations already produce immutable audit writes, and gateway
HTTP routes already declare whether the route requires audit. Composition roots
still need an application-level binding that proves the HTTP route maps to the
same authorized action before audit storage planning. Without that boundary, a
handler could persist an audit event for an action reached through the wrong
route, or accidentally bypass the route-level audit requirement.

## Decision

Add `plan_application_control_plane_with_audit_storage_from_http` and
`validate_control_plane_http_action_for_audit` to `prodex-application`.

The planner validates the canonical HTTP route/action mapping, verifies both the
route audit requirement and immutable action audit semantics, then delegates to
the existing append-only audit storage planner. Route, method, and mismatch
failures use stable redacted application error responses.

## Consequences

- Control-plane audit storage planning now has a single application entry point
  for HTTP adapters.
- Route/action mismatches fail before append-only audit commands are planned.
- Method and route errors preserve gateway HTTP compatibility while avoiding
  operation, tenant, or storage detail leaks.
