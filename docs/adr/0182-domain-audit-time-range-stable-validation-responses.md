# 0182: Domain Audit Time Range Stable Validation Responses

## Status

Accepted

## Context

Audit query and export endpoints need bounded time filters for retention,
incident response, and efficient storage scans. Without a domain-owned range
type, adapters can compare raw timestamps inconsistently or return errors that
echo rejected query values.

`AuditTimestamp` validates individual timestamps. Audit queries also need a
stable rule for start/end ordering and a redacted response plan when the range
is invalid.

## Decision

`prodex-domain` owns `AuditTimeRange` and
`plan_audit_time_range_error_response`.

`AuditTimeRange::new` accepts optional start and end timestamps, but rejects
ranges where the start is after the end. The response planner maps ordering
failures to a stable `audit_time_range_invalid` code and generic message without
echoing rejected timestamps, tenant identifiers, principal identifiers, resource
identifiers, audit chain material, or credential-like material.

## Consequences

- Gateway and control-plane adapters can validate audit query/export time
  filters consistently.
- Storage adapters receive an already checked range contract.
- Raw invalid range values remain available only to trusted diagnostics.
