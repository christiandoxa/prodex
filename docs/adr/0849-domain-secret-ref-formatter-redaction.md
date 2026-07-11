# ADR 0849: Redact domain secret reference formatters

Status: Accepted

## Context

`SecretRef` stores provider, name, and optional version fields for secret lookup.
Name and version were redacted, but `Debug` and `Display` still exposed the
provider part of the reference.

## Decision

Keep structured accessors for trusted secret-provider lookup, but redact the
provider in `Debug` output and render `Display` as a generic secret-reference
placeholder. The domain boundary guard pins the `SecretRef` type, its
`is_well_formed` shape check, and the redacted display placeholder so future
domain refactors cannot drop those contracts silently.

## Consequences

Secret lookup remains unchanged, while generic formatter output no longer
exposes any part of the secret reference.
