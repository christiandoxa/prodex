# 0184: Domain Audit Sort Order Stable Validation Responses

## Status

Accepted

## Context

Audit query and export endpoints need deterministic ordering for pagination,
incident response, and reproducible exports. If adapters accept free-form sort
strings, deployments can drift on supported fields and can leak rejected query
values in client-visible errors.

## Decision

`prodex-domain` owns `AuditSortOrder`,
`AuditSortOrder::parse`, and `plan_audit_sort_order_error_response`.

Accepted sort orders are exactly `occurred_at_asc` and `occurred_at_desc`.
Validation failures map to stable status/code/message response plans that do
not echo rejected sort strings, tenant identifiers, principal identifiers,
resource identifiers, audit chain material, or credential-like material.

## Consequences

- Gateway and control-plane adapters can validate audit query/export ordering
  consistently.
- Storage adapters receive a bounded ordering contract instead of raw sort
  strings.
- Raw invalid sort order values remain available only to trusted diagnostics.
