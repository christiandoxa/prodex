# 0219: HTTP Versioned Control-Plane Routes

## Status

Accepted.

## Context

The gateway already separates data-plane, health, and control-plane surfaces
before higher-level application adapters apply tenant isolation, idempotency,
and audit rules. Data-plane routes accept both legacy and `/v1` paths, but the
control-plane classifier only recognized legacy `/admin` and `/scim` prefixes.

Enterprise deployments need a stable versioned API surface for admin and SCIM
adapters without silently demoting `/v1/admin` or `/v1/scim` traffic to unknown
routes.

## Decision

Classify `/v1/admin...` and `/v1/scim...` as `ControlPlane` while preserving
the existing `/admin...` and `/scim...` prefixes.

The gateway remains side-effect-free: it only classifies the route kind. Tenant
authorization, idempotency storage, SCIM behavior, and audit emission stay in
the application/control-plane composition roots.

## Consequences

- Versioned control-plane HTTP adapters can share the same route governance as
  legacy admin and SCIM paths.
- Existing unversioned integrations remain compatible.
- Unknown route handling remains fail-closed for unrelated paths.
