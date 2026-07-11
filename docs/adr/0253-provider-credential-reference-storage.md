# ADR 0253: Provider-Credential Reference Storage

## Status

Accepted

## Context

Provider credentials must be managed by the control plane without storing raw
secret material in domain or storage models. The domain already exposes
redacted `SecretRef` values, but durable storage still needs a tenant-scoped
contract for provider credential references.

## Decision

Introduce `ProviderCredentialId` and a storage
`ProviderCredentialReferenceCommand`. PostgreSQL and SQLite plans persist only
the provider name plus structured `SecretRef` fields. The plans use
tenant-scoped upserts keyed by `(tenant_id, provider_name)` and keep
`(tenant_id, provider_credential_id)` as the primary key. Request-serving plans
emit DML only and reject cross-tenant storage keys before adapter execution.

## Consequences

Provider credential rotation can update durable secret references without
placing credential bytes in database rows, logs, or domain structs. Composition
roots can later combine this storage plan with control-plane authorization,
idempotency, and immutable audit writes.
