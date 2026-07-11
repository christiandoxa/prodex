# ADR 0959: Application Audit Export Debug Redaction

## Status

Accepted

## Context

Application audit-export plans carry control-plane action details, audit export
query metadata, audit digest state, authorized/denied decisions, and nested
storage planning. Derived debug output would expose tenant and audit-query
topology in diagnostics.

## Decision

Use custom `Debug` implementations for `ApplicationAuditExportRequest`,
`ApplicationAuditExportPlan`, and `ApplicationAuditExportError`. Redact control
plane action details, audit export query metadata, audit digests, decisions,
storage plans, route/resource mismatches, tenant mismatches, and nested storage
errors while preserving planner and error variant names.

## Consequences

Application diagnostics can distinguish audit-export planner shapes without
leaking tenant identifiers, audit digests, route topology, or query details.
