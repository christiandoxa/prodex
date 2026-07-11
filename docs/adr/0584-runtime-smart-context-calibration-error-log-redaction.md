# ADR 0584: Runtime Smart Context Calibration Error Log Redaction

## Status

Accepted

## Context

Smart-context token calibration persists runtime observations used by context
budgeting. In test and synchronous save paths, save failures are logged with a
detailed error chain. That chain can include filesystem paths, copied headers,
bearer tokens, key-bearing URLs, or storage diagnostic material.

The persisted calibration behavior should remain unchanged. The boundary to
harden is the runtime log field.

## Decision

Route smart-context token calibration save error log values through a local
helper that redacts secret-like text and keeps structured log fields single-line.

## Consequences

- Calibration save logs keep reason, stage, lag, and failure context.
- Secret-like bearer tokens and key assignments are removed before runtime log
  persistence.
- Calibration merge, save scheduling, and budgeting behavior are unchanged.
