# ADR 0388: Observability User Lifecycle Metric

## Status

Accepted.

## Context

Enterprise control planes need SCIM and user lifecycle telemetry for invite, create, update, and delete flows. Operators need to count authorization, denial, persistence, and failure outcomes without exposing tenant IDs, user IDs, email addresses, SCIM resource IDs, request payloads, role assignments, storage errors, or raw identity-provider details as metric labels.

## Decision

Add `plan_user_lifecycle_metric` to `prodex-observability`.

The planner emits `prodex_user_lifecycle_events_total`, increments by one, and uses only the closed enum labels `user_lifecycle_operation` and `user_lifecycle_result`.

## Consequences

- User invite and SCIM lifecycle adapters can publish create, update, delete, denial, persistence, and failure outcomes through one shared low-cardinality contract.
- Tenant IDs, user IDs, email addresses, SCIM resource IDs, role assignments, request payloads, identity-provider details, and raw storage errors remain trace/log/audit data subject to redaction.
- The observability boundary guard can enforce the user lifecycle telemetry contract before a concrete metrics backend is wired in.
