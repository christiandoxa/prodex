# 0196: Domain Audit Retention Hold

## Status

Accepted

## Context

Regulated deployments may need legal holds that prevent retention cleanup from
purging specific audit events. If storage adapters implement holds directly,
tenant checks, event timestamp validation, expiry handling, and redacted error
responses can drift across backends.

The domain already owns audit event identity, tenant scope, reason-code
validation, timestamp bounds, and retention cleanup planning.

## Decision

`prodex-domain` owns `AuditRetentionHold` and
`plan_audit_retention_hold_error_response`.

An audit retention hold binds a tenant, an `AuditEventId`, a validated
`AuditReasonCode`, and an optional expiry timestamp. `protects_event` fails
closed for cross-tenant events and invalid event timestamps, then reports
whether the hold is active and applies to the event.

## Consequences

- Retention cleanup can consult a domain-owned legal-hold contract before
  deleting events.
- Hold reasons reuse stable audit reason-code validation.
- Tenant IDs, event IDs, timestamps, hold internals, and storage details stay
  out of client-visible hold errors.
