# ADR 0269: Control-Plane Secret and Budget Authorization Boundaries

## Status

Accepted

## Context

Virtual key creation, virtual key secret rotation, provider credential rotation,
and budget policy updates are high-impact control-plane operations. They govern
tenant credentials, provider access, and spending limits. The control-plane
operation registry already assigns explicit resource/action pairs to these
operations, but direct `prodex-authz` callers could still use the generic admin
boundary.

Generic admin checks are too coarse for regulated deployments because they do
not prove that secret rotation and budget mutation were authorized against the
intended resource and action.

## Decision

Add explicit `prodex-authz` boundaries for:

- virtual key create;
- virtual key rotate secret;
- provider credential rotate;
- budget update.

Each boundary requires `CredentialScope::ControlPlane`, `Role::Admin`, tenant
access, and the exact `ResourceKind` plus `ResourceAction` used by the matching
`ControlPlaneOperation`.

Data-plane and break-glass scopes do not satisfy these normal admin operation
boundaries. Break-glass access remains a separate audited path.

## Consequences

Adapters can authorize credential and budget operations without falling back to
generic admin checks. Secret-bearing operations remain aligned with `SecretRef`
storage composition, and stable redacted authorization responses remain
unchanged.
