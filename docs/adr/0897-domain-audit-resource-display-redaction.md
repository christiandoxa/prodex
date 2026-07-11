# ADR 0897: Domain audit resource display redaction

## Status

Accepted.

## Context

Audit action, resource-kind, and resource-id response planners already expose
stable messages. Raw `Display` output still distinguished empty, too-long,
empty-segment, and invalid-character validation classes.

## Decision

Render `AuditActionError`, `AuditResourceKindError`, and `AuditResourceIdError`
with the same messages used by their response planners. Keep typed variants and
response codes unchanged for trusted classification.

Regression coverage pins exact display strings and rejects length/classification
wording from raw display output. Wrapper timestamp display expectations were
also aligned with the stable timestamp display contract introduced by ADR 0895.

## Consequences

Audit write/query boundaries can safely fall back to raw display strings without
exposing audit metadata validation shape. Diagnostics should continue matching
typed variants for exact reasons.
