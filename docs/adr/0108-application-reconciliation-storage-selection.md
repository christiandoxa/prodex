# ADR 0108: Application Reconciliation Storage Selection

## Status

Accepted

## Context

The application boundary already plans post-upstream usage reconciliation, while
PostgreSQL and SQLite crates expose driver-free DML plans for durable
commit/release accounting. Composition roots should not reimplement storage
adapter selection for this security-sensitive accounting path.

## Decision

`prodex-application` now accepts an `ApplicationUsageReconciliationRequest`
containing the durable store kind and gateway reconciliation request. It returns
both:

- the gateway reconciliation plan with trace-aware spans;
- a durable storage reconciliation plan for PostgreSQL or SQLite.

The application crate still does not execute SQL and remains free of database
drivers, HTTP frameworks, async runtimes, filesystem access, network clients,
and provider SDKs.

## Consequences

- Composition roots get one use-case plan for post-provider accounting and
  durable DML selection.
- PostgreSQL remains the production source-of-truth target, while SQLite stays
  available for local compatibility.
- Cross-tenant reconciliation is rejected before adapter execution.
