# ADR 0935: Storage virtual-key secret reference debug redaction

## Status

Accepted.

## Context

Virtual-key secret reference commands and plans carry tenant IDs, virtual-key
IDs, principal IDs, display names, secret references, mutation kinds, and
timestamps. These fields are needed by storage adapters but should not appear in
generic debug output.

## Decision

Use custom `Debug` implementations for `VirtualKeySecretReferenceCommand` and
`VirtualKeySecretReferencePlan`. Redact storage keys, tenant IDs, virtual-key
IDs, principal IDs, display names, secret references, and timestamps while
preserving mutation kind.

Regression coverage rejects tenant, virtual-key, principal, display-name,
secret-reference, and timestamp values in rendered debug output.

## Consequences

Storage diagnostics can identify virtual-key secret reference command/plan shape
without exposing credential reference details. Planning behavior is unchanged.
