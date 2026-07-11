# ADR 0158: Backup and restore stable error responses

## Status

Accepted

## Context

The domain backup/restore boundary validates backup identifiers and restore
plans before operators can execute recovery workflows. Raw restore errors can
expose whether a backup is failed, expired, missing checksums, or mismatched
against an expected checksum. Those details are useful in trusted runbooks,
restore drill evidence, and operator diagnostics, but client-visible
startup/readiness or control-plane APIs should return stable redacted error
envelopes.

## Decision

Add stable response planners to `prodex-domain`:

- `plan_backup_id_error_response` for invalid backup identifiers; and
- `plan_restore_error_response` for non-restorable backups and checksum
  validation failures.

The planners expose deterministic status/code/message triples while excluding
backup IDs, timestamps, checksum values, and backend-specific restore internals.

## Consequences

Control-plane and operational composition roots can expose machine-readable
backup/restore failures while retaining raw errors for trusted diagnostics,
restore drill records, and audit trails. This advances the enterprise DoD item
that backup and restore behavior be testable and safely reportable.
