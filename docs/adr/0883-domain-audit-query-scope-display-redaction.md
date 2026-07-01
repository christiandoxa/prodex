# ADR 0883: Domain audit query scope display redaction

## Status

Accepted.

## Context

Audit query scope checks fail closed for cross-tenant events and already expose a
stable response planner. The raw `Display` string still named the tenant-scope
classification, and `AuditQueryPlanError::Scope` delegated that same string.

## Decision

Render `AuditQueryScopeError` as `audit query scope is invalid`, matching the
response planner message. Keep the typed `CrossTenantEvent` variant unchanged so
trusted diagnostics, audit, and metrics can still classify the denial.

Regression coverage pins the display string directly and through
`AuditQueryPlanError::Scope`, while rejecting tenant wording in raw display
output.

## Consequences

Audit query and export boundaries can safely fall back to raw display strings
without exposing tenant-isolation classification. Structured code should keep
matching variants when it needs the exact reason.
