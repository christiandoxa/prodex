# ADR 0379: Observability Provider Capability Negotiation Metric

## Status

Accepted.

## Context

Enterprise provider routing must preserve model and provider capability negotiation while avoiding high-cardinality metric labels. Operators need to count compatible, incompatible, and no-candidate negotiation outcomes for supported capability kinds without exposing model names, endpoints, tenant IDs, request IDs, or raw negotiation errors as labels.

## Decision

Add `plan_provider_capability_negotiation_metric` to `prodex-observability`.

The planner emits `prodex_provider_capability_negotiation_events_total`, increments by one, and uses only the closed enum labels `provider`, `provider_capability`, and `provider_capability_result`.

## Consequences

- Provider routers can publish capability negotiation outcomes through a shared low-cardinality contract.
- Model names, endpoints, tenants, request IDs, and raw negotiation errors remain trace/log/report data subject to redaction.
- The observability boundary guard can enforce the provider capability telemetry contract before a concrete metrics backend is wired in.
