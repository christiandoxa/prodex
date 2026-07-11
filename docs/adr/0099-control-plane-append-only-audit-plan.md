# ADR 0099: Control-Plane Append-Only Audit Plan

## Status

Accepted

## Context

Enterprise control-plane actions are security-sensitive. Tenant changes,
identity lifecycle operations, key rotation, policy publication, budget edits,
configuration publication, break-glass access, and denied authorization attempts
must produce immutable audit evidence. A loose `AuditEvent` value is not enough
for adapters because it does not state whether the event is advisory telemetry
or a required durable write.

## Decision

`prodex-control-plane` returns a `ControlPlaneAuditWritePlan` with each
authorized and denied control-plane decision. The plan contains:

- the canonical `AuditEvent`;
- the tenant partition key;
- an explicit `AppendOnlyHashChain` write mode.

The crate remains HTTP-, database-, filesystem-, and runtime-neutral. Concrete
adapters are responsible for persisting the plan in an append-only audit store
before treating the control-plane action as durably completed.

## Consequences

- Control-plane adapters can no longer accidentally treat audit emission as an
  optional log side effect.
- Denied security-sensitive requests carry the same append-only audit
  requirement as successful mutations.
- The domain `AuditEnvelope` hash-chain model remains the durable verification
  target, while this crate only plans the required write.
