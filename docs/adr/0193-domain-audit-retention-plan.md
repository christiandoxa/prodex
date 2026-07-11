# 0193: Domain Audit Retention Plan

## Status

Accepted

## Context

Retention cleanup is security-sensitive in regulated deployments. Storage
adapters should not decide independently which audit events are eligible for
purge because that can drift from tenant scoping and timestamp validation used
by query/export paths.

The domain already owns audit tenant scope, timestamp validation, and retention
policy bounds. It needs a small plan type that computes the cutoff and fails
closed before storage deletion code receives purge candidates.

## Decision

`prodex-domain` owns `AuditRetentionPlan` and
`plan_audit_retention_plan_error_response`.

`AuditRetentionPlan` combines `AuditQueryScope`, `AuditRetentionPolicy`, and a
validated current `AuditTimestamp`. It computes the cutoff in days and exposes
`is_event_expired`, which rejects cross-tenant events and invalid event
timestamps before reporting whether an event is older than the cutoff.

## Consequences

- Retention cleanup can share the same tenant and timestamp boundary as audit
  query/export paths.
- Storage adapters receive only a domain decision about purge eligibility.
- Tenant IDs, audit event IDs, timestamps, and retention internals remain out of
  client-visible retention-plan errors.
