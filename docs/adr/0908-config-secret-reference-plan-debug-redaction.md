# ADR 0908: Config secret reference plan debug redaction

## Status

Accepted.

## Context

Configuration secret publication accepts `SecretRef` values instead of raw
secret material. `SecretRef` itself redacts provider, name, and version in
formatters, but `ConfigSecretReferencePlan` used derived `Debug` and could
expose tenant context through generic diagnostics.

## Decision

Use a custom `Debug` implementation for `ConfigSecretReferencePlan` that
redacts tenant IDs and delegates secret reference formatting to the existing
`SecretRef` redaction contract. Keep exact fields available through typed
access for publication planning.

Regression coverage rejects tenant IDs and secret reference parts in rendered
plan debug output.

## Consequences

Configuration composition roots can log plan shape without exposing tenant
identifiers or secret reference material. Secret lookups still receive exact
typed references.
