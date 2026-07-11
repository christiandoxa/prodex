# 0651: Audit Sort Order Parse Exact Boundary

## Status

Accepted

## Context

`AuditSortOrder::parse` trimmed input before matching query/export sort order
values. Sort order is stable low-cardinality API metadata, so accepting padded
values can hide malformed callers.

## Decision

`AuditSortOrder::parse` now matches the exact input value. Whitespace-only
values, leading whitespace, and trailing whitespace are treated as unknown
input. Only zero-length values are empty errors.

## Consequences

- Audit query and export sort order parsing stays exact at the domain boundary.
- Existing valid values keep working.
- Stable audit error responses continue to redact rejected raw values.
