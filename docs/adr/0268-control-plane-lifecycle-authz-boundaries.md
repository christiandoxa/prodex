# ADR 0268: Control-Plane Lifecycle Authorization Boundaries

## Status

Accepted

## Context

Tenant, user, role-binding, and service-identity lifecycle operations are
security-sensitive control-plane actions. The control-plane crate already
models these operations with explicit resource/action pairs, but adapters that
use `prodex-authz` directly could still fall back to a generic admin boundary.

Generic admin authorization is too coarse for enterprise audits because it
cannot prove that create, update, delete, grant, and revoke paths were checked
against the intended resource kind before storage mutation.

## Decision

Add explicit `prodex-authz` boundaries for core control-plane lifecycle
operations:

- tenant create and update;
- user create, update, and delete;
- role-binding grant and revoke;
- service-identity create.

Each boundary requires `CredentialScope::ControlPlane`, `Role::Admin`, tenant
access, and the exact `ResourceKind` plus `ResourceAction` used by the matching
`ControlPlaneOperation`.

Data-plane and break-glass scopes do not satisfy these normal lifecycle
boundaries. Break-glass access remains a separate audited path.

## Consequences

Lifecycle adapters can authorize against precise resource/action contracts
before durable storage mutation. Stable redacted authorization responses remain
unchanged, and generic admin authorization stays available only as a broad
fallback boundary rather than the preferred lifecycle contract.
