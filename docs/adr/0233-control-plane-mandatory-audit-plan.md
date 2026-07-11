# 0233: Control-Plane Mandatory Audit Plan

## Status

Accepted

## Context

Regulated deployments require every security-sensitive control-plane action to
produce an immutable audit event. The control-plane boundary already emits
append-only audit write plans for authorized and denied decisions, but new
operations also need an explicit requirement contract so adapters and tests do
not treat audit emission as optional or best-effort.

## Decision

`prodex-control-plane` owns `ControlPlaneAuditRequirementPlan` and exposes it
through `ControlPlaneOperation::audit_requirement`.

Every current control-plane operation requires:

- append-only hash-chain audit writes;
- audit on successful authorization;
- audit on denied authorization.

`ControlPlaneOperation::ALL` gives tests and adapters a stable operation
registry for checking coverage as the control-plane surface grows.

## Consequences

- New control-plane operations must choose an audit action and immutable audit
  requirement before compiling cleanly against the operation registry tests.
- Storage and application adapters can reject or defer completion until the
  planned append-only audit write is durably selected.
- Audit requirements remain side-effect-free and HTTP/storage neutral.
