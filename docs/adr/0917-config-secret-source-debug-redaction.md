# ADR 0917: Config secret source debug redaction

## Status

Accepted.

## Context

Configuration secret sources represent either a `SecretRef` or rejected raw
secret material. `SecretRef` formatting is redacted, but derived
`ConfigSecretSource` debug output delegated directly to the inner reference and
left the boundary dependent on nested formatter behavior.

## Decision

Use a custom `Debug` implementation for `ConfigSecretSource` that redacts
reference values at the source boundary. Keep exact references available through
typed variants for publication planning.

Regression coverage rejects provider, secret name, and version values in
rendered source debug output.

## Consequences

Configuration diagnostics can inspect source shape without exposing secret
reference metadata. Secret publication behavior remains unchanged.
