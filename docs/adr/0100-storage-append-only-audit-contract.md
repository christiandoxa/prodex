# ADR 0100: Storage Append-Only Audit Contract

## Status

Accepted

## Context

Enterprise control-plane decisions now produce an explicit audit write plan for
security-sensitive actions. Storage adapters need a matching driver-neutral
contract so audit events are not persisted as best-effort logs or mutable rows.

The contract must preserve tenant isolation and support hash-chain verification
without forcing the storage boundary crate to depend on a database driver,
filesystem, async runtime, HTTP framework, or cryptographic implementation.

## Decision

`prodex-storage` defines `AppendOnlyAuditCommand`,
`AppendOnlyAuditPlan`, and `plan_append_only_audit`. The plan:

- requires a tenant-scoped storage key;
- rejects cross-tenant event/key mismatches;
- wraps the event in a domain `AuditEnvelope`;
- records `HashChain` as the append mode.

Concrete Postgres or SQLite adapters are responsible for enforcing append-only
writes, previous-digest uniqueness or compare-and-set semantics, and durable
transaction boundaries.

## Consequences

- Control-plane and storage boundaries now agree that audit persistence is a
  required durable append, not optional telemetry.
- Tenant partitioning is validated before adapter-specific persistence.
- Database adapters can implement hash-chain concurrency controls while the
  core storage contract stays side-effect-free and testable.
