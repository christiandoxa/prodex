# ADR 0264: Billing Ledger Read Composition

## Status

Accepted

## Context

The control plane exposes `BillingRead` as a viewer-eligible operation, and the
data plane already writes tenant-scoped usage ledger events. The missing
boundary was a durable, tenant-scoped billing read plan that composes
authorization, immutable audit, and backend query selection before adapters read
`prodex_usage_ledger`.

Without that boundary, billing screens or exports could reimplement tenant
predicates, time ranges, and limits outside the shared application layer.

## Decision

Add `BillingLedgerQueryCommand` and `plan_billing_ledger_query` to
`prodex-storage`. The command carries tenant storage key, tenant ID, optional
time range bounds, page limit, and sort order. It rejects cross-tenant storage
keys and invalid ranges behind stable redacted error responses.

PostgreSQL and SQLite expose request-path SELECT plans for
`prodex_usage_ledger`. The SQL requires `tenant_id`, optional time range
filters, stable `occurred_at` ordering, and explicit limits. PostgreSQL keeps
the RLS tenant context before the SELECT. No request-path DDL is introduced.

Add `plan_application_billing_read` to `prodex-application`. The planner
accepts only `BillingRead` on `ResourceKind::Billing`, verifies action/query
tenant alignment, always plans append-only audit storage, and only plans
billing ledger query storage after an authorized decision.

## Consequences

Billing read paths now share one authorization, audit, and query-planning
boundary. Client-visible errors stay redacted and do not expose tenant IDs,
time bounds, usage amounts, ledger internals, or SQL details.
