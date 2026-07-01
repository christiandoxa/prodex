# ADR 0889: Domain migration plan display redaction

## Status

Accepted.

## Context

Migration plan response planners already expose stable messages for request-path
execution, missing migrator lock, unresolved failed steps, and invalid
expand/backfill/verify/contract ordering. Raw `Display` output still used
implementation wording such as lock owner and individual step names.

## Decision

Render `MigrationPlanError` variants with the same stable messages as the
response planner. Keep typed variants and response codes unchanged so callers can
classify exact migration failures for audit and operations.

Regression coverage pins the display strings and rejects lock-owner plus
specific step-order wording from raw display output.

## Consequences

Migration boundaries can safely fall back to raw display strings without
exposing execution topology or detailed step ordering. Trusted diagnostics
should continue matching typed variants.
