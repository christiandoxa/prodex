# ADR 0091: Control Plane Planning Boundary

## Status

Accepted

## Context

Prodex is being refactored into an enterprise-ready modular monolith with a
stateless data-plane gateway and a separate control-plane boundary for tenant,
identity, key, billing, audit, policy, and configuration management. The legacy
application crate still contains many admin HTTP handlers, so the next safe step
is to introduce a side-effect-free planning crate before moving transport or
storage code.

The control plane must not become a data-plane bypass. Data-plane credentials
must not call admin operations, control-plane credentials must not grant
inference access, and break-glass credentials must be separate, short-lived, and
audited.

## Decision

Add `prodex-control-plane` as an HTTP-neutral crate. It plans control-plane
actions by enforcing:

- tenant-scoped resource access;
- strict `ControlPlane` credential scope for normal admin actions;
- explicit role requirements per operation;
- separate `BreakGlass` scope with non-empty reason and expiry;
- immutable audit events for both authorized and denied decisions;
- configuration publication checks through the revisioned `prodex-config`
  boundary.

Add `scripts/ci/control-plane-boundary-guard.mjs` and include it in preflight so
this crate cannot depend on HTTP frameworks, async runtimes, database drivers,
network clients, filesystem/environment/process access, transports, or provider
SDKs.
The npm script and local preflight run the guard self-test before the workspace
scan so forbidden-dependency/source and unsafe-forbid checks cannot silently rot.

## Consequences

Admin HTTP handlers can be migrated incrementally to call this planning boundary
without changing wire compatibility in the same patch. Storage adapters remain
responsible for persisting audit events and applying tenant-scoped writes, while
this crate remains deterministic and unit-testable. Future control-plane routes
must add operation-specific tests here before adapter wiring.
