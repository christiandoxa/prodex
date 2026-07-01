# 0231: Config Secret Reference Plan

## Status

Accepted.

## Context

Enterprise configuration must not embed raw provider credentials, webhook
secrets, OIDC client secrets, or data-plane credentials in domain payloads.
Configuration can point at secret material, but actual secret bytes must remain
behind a secret provider boundary.

## Decision

Add `ConfigSecretSource`, `ConfigSecretReferencePlan`, and
`plan_config_secret_reference` to `prodex-config`.

The planner accepts only `SecretRef` sources and rejects raw secret material with
`ConfigSecretReferenceError::RawSecretMaterialRejected`. The raw-material variant
is a marker only; it does not store or expose the rejected value.

## Consequences

Control-plane publication and gateway activation flows can validate that secret
settings carry references instead of embedded secret values. Actual secret
resolution remains in the secret provider boundary, and public errors stay
stable and redacted.
