# 0192: Domain Audit Retention Policy

## Status

Accepted

## Context

Regulated deployments need bounded audit retention configured through the
control plane. If adapters pass raw retention values directly to storage, the
minimum retention period can drift across backends and invalid values can leak
through client-visible errors.

The domain can validate the retention window without depending on storage,
HTTP, filesystem, or provider crates.

## Decision

`prodex-domain` owns `AuditRetentionPolicy` and
`plan_audit_retention_policy_error_response`.

The policy defaults to 365 days, requires at least 30 days, and allows at most
3,650 days. Invalid values map to one stable,
`audit_retention_policy_invalid`, redacted response plan.

## Consequences

- Control-plane audit settings can share one retention validation contract.
- Storage adapters receive bounded retention days instead of raw user input.
- Raw retention values, tenant IDs, and storage implementation details stay out
  of client-visible audit retention errors.
