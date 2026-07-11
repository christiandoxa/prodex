# 0191: Domain Audit Page Results

## Status

Accepted

## Context

Audit query and export adapters need to return page items and the next cursor
using the same ordering, tenant scope, timestamp validation, and page limit
rules. `AuditQueryPlan::select_events_after` applies cursor position, but
adapters still had to decide whether another page exists and how to produce the
next cursor.

That cursor-generation logic is part of the audit compatibility contract and
should not drift across control-plane HTTP, export, or storage adapters.

## Decision

`AuditQueryPlan` owns `page_events` and `AuditExportPlan` delegates to it.

The helper applies cursor-aware selection, returns the domain `Page` envelope,
and emits the next `AuditQueryCursor` only when more matching events remain.
Errors use `AuditQueryPageError`, which delegates to the existing redacted
query-plan, timestamp, and cursor response planners.

## Consequences

- Audit query and export adapters share one page-result contract.
- Next cursor creation uses the same timestamp and sort-order validation as
  cursor parsing.
- Raw cursor strings, timestamps, event IDs, tenant IDs, and parser internals
  remain out of audit page errors.
