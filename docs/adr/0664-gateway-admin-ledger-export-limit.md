# 0664: Gateway Admin Ledger Export Limit

## Status

Accepted

## Context

Gateway admin CSV and billing-summary exports loaded the billing ledger with
`usize::MAX`. That made file, SQL, and Redis compatibility paths capable of
unbounded reads in a control-plane request, which conflicts with the enterprise
requirement for bounded request-serving work.

## Decision

Use an explicit `RUNTIME_GATEWAY_ADMIN_LEDGER_EXPORT_LIMIT` for CSV and summary
ledger exports. The JSON ledger endpoint keeps its existing smaller page limit.

## Consequences

- Admin ledger export work is bounded in request-serving paths.
- Existing response formats stay unchanged.
- Larger historical exports need a future paginated or offline export flow.
