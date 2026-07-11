# 0410. Observability enterprise ID metric

## Status

Accepted

## Context

Enterprise readiness requires typed globally unique IDs for tenants,
principals, requests, calls, reservations, virtual keys, policy revisions, and
audit events. Operators still need aggregate visibility into ID generation and
parse rejection without exposing high-cardinality IDs or UUID parser internals
as metric labels.

## Decision

Add `plan_enterprise_id_metric` to `prodex-observability`.

The planner emits `prodex_enterprise_id_events_total`, increments by one, and
uses only `enterprise_id_kind` and `enterprise_id_result` labels. ID values,
tenant identifiers, principal identifiers, request IDs, call IDs, reservation
IDs, virtual key IDs, policy revision IDs, audit event IDs, and raw parser
errors remain trace/log data only.

## Consequences

Adapters can publish aggregate ID-generation and parse-rejection counters
without bypassing the metric cardinality boundary.

Rollback is to stop calling the planner; no storage migration is involved.
