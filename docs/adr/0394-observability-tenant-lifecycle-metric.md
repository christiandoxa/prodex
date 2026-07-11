# ADR 0394: Observability Tenant Lifecycle Metric

## Status

Accepted.

## Context

Tenant create and update operations are foundational control-plane mutations for multi-tenant deployments. Operators need to count creation, update, authorization, denial, persistence, and failure outcomes without exposing tenant IDs, tenant display names, request payloads, caller identity, or raw storage errors as metric labels.

## Decision

Add `plan_tenant_lifecycle_metric` to `prodex-observability`.

The planner emits `prodex_tenant_lifecycle_events_total`, increments by one, and uses only the closed enum labels `account_lifecycle_operation` and `account_lifecycle_result`. The label keys intentionally avoid `tenant` because raw tenant identifiers are forbidden in metric labels by the domain telemetry boundary.

## Consequences

- Tenant lifecycle adapters can publish create, update, denial, persistence, and failure outcomes through one shared low-cardinality contract.
- Tenant IDs, tenant display names, request payloads, caller identity, and raw storage errors remain trace/log/audit data subject to redaction.
- The observability boundary guard can enforce the tenant lifecycle telemetry contract before a concrete metrics backend is wired in.
