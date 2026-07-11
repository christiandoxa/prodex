# ADR 0824: Redact domain policy digest debug output

Status: Accepted

## Context

`PolicyDigest` stores opaque policy integrity metadata. Derived `Debug` output
exposed the digest string through diagnostics even though policy signatures were
already redacted.

## Decision

Use a custom `Debug` implementation for `PolicyDigest` that redacts the digest
value.

## Consequences

Diagnostics can still identify digest-bearing values, but raw policy digest
material no longer appears through `PolicyDigest` debug output.
