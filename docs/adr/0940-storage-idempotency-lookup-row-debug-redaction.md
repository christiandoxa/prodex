# ADR 0940: Storage Idempotency Lookup Row Debug Redaction

## Status

Accepted

## Context

Idempotency lookup rows carry tenant identifiers, replay keys, request
fingerprints, timestamps, and completed response bodies loaded from durable
storage. Derived debug output would expose those values in diagnostics.

## Decision

Use a custom `Debug` implementation for `IdempotencyRecordLookupRow`. Redact
tenant, idempotency-key, fingerprint, timestamp, and response-body fields while
preserving row status.

## Consequences

Storage diagnostics can distinguish pending and completed lookup rows without
leaking tenant identifiers, replay keys, request fingerprints, timestamps, or
completed response payloads.
