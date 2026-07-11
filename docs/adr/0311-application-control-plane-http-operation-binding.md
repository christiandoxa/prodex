# ADR 0311: Application Control-Plane HTTP Operation Binding

## Status

Accepted.

## Context

`prodex-gateway-http` can classify admin and SCIM method/path pairs, but
composition roots still need to bind that HTTP decision to the canonical
`ControlPlaneOperation`. Without that binding, an adapter could accidentally
derive an idempotency fingerprint for one route while executing a different
control-plane action.

## Decision

`prodex-application` now exposes `plan_application_control_plane_http_route`.
It translates `GatewayControlPlaneOperation` into `ControlPlaneOperation` and
preserves the HTTP route plan's idempotency and audit flags.

The HTTP idempotency helpers reject route/action mismatches before reading
replay state or accepting a mutating idempotency key, including adapters that
provide a precomputed request fingerprint instead of a body digest.

## Consequences

Legacy admin and SCIM adapters can migrate one route at a time while sharing a
single operation binding. Route mismatch errors use a stable redacted response
so operation names, tenant IDs, and replay details are not exposed to clients.
