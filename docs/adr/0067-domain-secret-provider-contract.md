# ADR 0067: Domain secret provider contract

## Status

Accepted.

## Context

The enterprise target requires secret references instead of raw secret strings in
domain objects, plus development and production secret-provider adapters with
rotation support. `prodex-domain` already had `SecretRef`, but it did not define
provider resolution, redacted secret material, or rotation metadata. Without a
pure domain contract, application code can drift toward ad hoc raw `String`
secrets and provider-specific coupling.

## Decision

Add pure domain types for secret resolution: `SecretProvider`,
`SecretProviderDescriptor`, `SecretResolutionRequest`, redacted `SecretMaterial`,
`SecretPurpose`, `SecretRotationPolicy`, and `SecretRotationStatus`. The contract
is side-effect-free from the domain crate's perspective and carries no
filesystem, environment, network, database, or provider SDK dependencies.
Development env/file and external secret-manager providers are represented as
descriptor kinds; concrete adapters will live outside `prodex-domain`.

## Consequences

- Domain and application boundaries can require `SecretRef` and provider
  contracts without exposing raw secret values through debug/display output.
- Rotation metadata can be validated and audited before concrete adapters exist.
- Future adapter crates must implement the trait outside the domain boundary and
  keep actual secret I/O out of pure domain code.
