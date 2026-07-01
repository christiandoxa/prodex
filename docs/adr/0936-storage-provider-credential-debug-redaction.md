# ADR 0936: Storage provider credential debug redaction

## Status

Accepted.

## Context

Provider credential reference commands and plans carry tenant IDs, provider
credential IDs, provider names, secret references, storage keys, and rotation
timestamps. These fields are needed by storage adapters but should not appear in
generic debug output.

## Decision

Use custom `Debug` implementations for `ProviderCredentialReferenceCommand` and
`ProviderCredentialReferencePlan`. Redact storage keys, tenant IDs, provider
credential IDs, provider names, secret references, and rotation timestamps.

Regression coverage rejects tenant, credential, provider, secret-reference, and
timestamp values in rendered provider credential debug output.

## Consequences

Storage diagnostics can identify provider credential reference command/plan
shape without exposing provider or secret-reference details. Planning behavior is
unchanged.
