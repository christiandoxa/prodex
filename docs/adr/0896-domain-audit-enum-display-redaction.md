# ADR 0896: Domain audit enum display redaction

## Status

Accepted.

## Context

Audit outcome, export-format, and sort-order response planners already expose
stable messages. Raw `Display` output still distinguished empty from unknown
values for these request-controlled enums.

## Decision

Render `AuditOutcomeError`, `AuditExportFormatError`, and `AuditSortOrderError`
with the same messages used by their response planners. Keep typed variants and
response codes unchanged for trusted classification.

Regression coverage pins exact display strings and rejects unknown/empty
classification from raw display output.

## Consequences

Audit query and export boundaries can safely fall back to raw display strings
without exposing enum validation shape. Diagnostics should continue matching
typed variants for exact reasons.
