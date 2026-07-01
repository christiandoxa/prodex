# 0618. Config SecretRef Shape Guard

## Status

Accepted

## Context

Enterprise configuration must carry secret references, not raw secret material.
The config boundary already rejected raw secret material, but a `SecretRef`
with empty or whitespace-containing provider, name, or version parts could still
enter a publication plan.

That can create ambiguous secret-provider lookups and weak audit evidence.

## Decision

`SecretRef` now exposes `is_well_formed()`. `prodex-config` rejects malformed
secret references before producing `ConfigSecretReferencePlan`.

The rejection uses a stable, redacted
`configuration_secret_reference_invalid` response.

## Consequences

- Secret-bearing config must point to a non-empty provider and non-empty secret
  name.
- Whitespace in provider, name, or version is rejected at the configuration
  boundary.
- Existing `SecretRef::new` call sites remain source-compatible; validation
  happens where a reference crosses into config publication planning.
