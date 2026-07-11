# ADR 0906: Storage idempotency lookup row display redaction

## Status

Accepted.

## Context

Idempotency lookup rows are loaded from durable storage before replay decisions.
The row materializer rejects tenant mismatches, key mismatches, invalid
fingerprints, and incomplete completed rows. Its response mapper already
exposes one stable message, but raw `Display` output still distinguished row
shape failures.

## Decision

Render `IdempotencyRecordLookupRowError` with the same message used by
`materialize_idempotency_record_lookup_row_error_response`. Keep typed variants
unchanged for trusted classification.

Regression coverage pins exact display strings for tenant mismatch, key
mismatch, invalid fingerprint, and missing completion metadata.

## Consequences

Storage adapters and composition roots can safely fall back to raw display
strings without exposing durable replay-row shape. Diagnostics should continue
matching typed variants for exact reasons.
