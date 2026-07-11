# ADR 0248: Control-plane operation lifecycle registry

## Status

Accepted

## Context

The control-plane boundary enumerates tenant, identity, virtual-key, policy,
provider-credential, budget, billing, audit, and configuration operations. Each
operation must bind to one expected resource kind, action, role requirement,
idempotency policy, and immutable audit behavior.

If new operations are added without registry coverage, an adapter could route a
lifecycle operation to the wrong resource kind or fail to audit denials.

## Decision

`prodex-control-plane` keeps `ControlPlaneOperation::ALL` as the operation
registry. Tests now assert that every operation has an explicit lifecycle
requirement and that every resource-kind mismatch is denied with an append-only
audit event.

## Consequences

Adding a control-plane operation requires updating the lifecycle registry test.
The boundary continues to fail closed on wrong resource kinds and records denial
evidence before adapter-specific storage writes are attempted.
