# ADR 0918: Storage virtual-key secret-reference display redaction

## Status

Accepted.

## Context

Storage planning for virtual-key secret references rejects requests when the
tenant storage key and request virtual-key ID disagree. The response planner
already returns a stable redacted message, but raw `Display` formatting included
the mismatched virtual-key identifiers.

## Decision

Use the same stable message for `VirtualKeySecretReferencePlanError`
tenant-mismatch and virtual-key-mismatch display output. Keep exact identifiers
available in typed error variants for storage adapters and tests.

Regression coverage verifies that virtual-key mismatch display output uses the
generic storage-boundary message.

## Consequences

Stringified storage errors no longer expose virtual-key topology. Structured
storage planning behavior and response codes remain unchanged.
