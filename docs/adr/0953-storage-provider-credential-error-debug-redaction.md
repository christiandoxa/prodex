# ADR 0953: Storage Provider Credential Error Debug Redaction

## Status

Accepted

## Context

Provider credential reference planner errors can carry tenant identifiers when
storage keys do not match credential reference request tenants. Display output
is generic, but derived debug output would expose the mismatched tenant values.

## Decision

Use a custom `Debug` implementation for `ProviderCredentialReferencePlanError`.
Redact tenant identifiers while preserving the error variant name.

## Consequences

Storage diagnostics can distinguish provider credential reference planner
failures without leaking tenant identifiers from mismatch errors.
