# ADR 0573: Redact token calibration save errors

## Status

Accepted

## Context

Smart-context token calibration metadata is saved by a background worker. Save
failures were written to runtime logs with detailed `{err:#}` chains. Those
chains can include local paths, environment diagnostics, bearer tokens, or
key-bearing URLs from lower-level storage helpers.

## Decision

Pass token calibration save error chains through the shared secret-like text
redactor before placing them in structured runtime log `error` fields.

## Consequences

- Runtime logs still include token calibration save failure metadata such as
  reason, lag, stage, and sample count.
- Secret-like material is removed from token calibration save error details.
- Token calibration persistence and scheduling behavior are unchanged.
