# 0650: Audit Enum Parse Exact Boundary

## Status

Accepted

## Context

Audit outcome and export-format parsers trimmed input before matching stable
machine-readable values. Those fields are low-cardinality API and audit
metadata, so silently accepting padded values hides malformed callers.

## Decision

`AuditOutcome::parse` and `AuditExportFormat::parse` now match the exact input
value. Only zero-length values are empty errors; whitespace-only, leading, or
trailing whitespace values are treated as unknown input.

## Consequences

- Audit enum parsing stays exact at the domain boundary.
- Existing valid values keep working.
- Stable audit error responses continue to redact rejected raw values.
