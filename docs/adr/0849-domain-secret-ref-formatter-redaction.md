# ADR 0849: Redact domain secret reference formatters

Status: Accepted

## Context

`SecretRef` stores provider, name, and optional version fields for secret lookup.
Name and version were redacted, but `Debug` and `Display` still exposed the
provider part of the reference.

## Decision

Keep structured accessors for trusted secret-provider lookup, but redact the
provider in `Debug` output and render `Display` as a generic secret-reference
placeholder.

## Consequences

Secret lookup remains unchanged, while generic formatter output no longer
exposes any part of the secret reference.
