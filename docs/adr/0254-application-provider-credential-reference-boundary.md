# ADR 0254: Application Provider-Credential Reference Boundary

## Status

Accepted

## Context

Provider credential references now have tenant-scoped storage contracts, but
composition roots still need an application-level planner. Without that layer,
HTTP adapters could select PostgreSQL or SQLite storage plans directly and
duplicate backend-specific logic around provider credential rotation.

## Decision

Add `plan_application_provider_credential_reference` to `prodex-application`.
The planner accepts a durable store selection and a
`ProviderCredentialReferenceCommand`, then selects the matching PostgreSQL or
SQLite DML plan. Errors are mapped to a redacted
`provider_credential_storage_unavailable` response.

## Consequences

Control-plane provider credential rotation can compose authorization,
idempotency, audit, and durable `SecretRef` storage as explicit application
steps. HTTP adapters do not need to know adapter-specific provider credential
SQL details or expose secret reference names in error responses.
