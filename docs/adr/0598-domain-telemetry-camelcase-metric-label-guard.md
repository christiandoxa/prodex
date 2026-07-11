# ADR 0598: Domain Telemetry CamelCase Metric Label Guard

## Status

Accepted.

## Context

The domain telemetry boundary rejects high-cardinality metric label keys such
as `tenant_id`, `request_id`, and `api_key`. The guard normalized kebab-case
and dotted keys, but camelCase keys such as `tenantId` or `requestId` could
avoid the forbidden-key check.

## Decision

Metric label key validation now checks both separator-normalized and compact
forms. This keeps snake_case, kebab-case, dotted, and camelCase forms of
forbidden identifiers out of metric labels.

## Consequences

Raw tenant, user, principal, key, request, call, and prompt identifiers remain
trace-only or redacted trace-only. Existing low-cardinality labels such as
`status_class` and `api_route` remain valid.
