# ADR 0692: Secret Backend Kind Rejects Whitespace Padding

## Status

Accepted.

## Context

`SecretBackendKind::from_str` parses the policy-facing secret backend kind.
The parser accepted leading and trailing whitespace before matching `file` or
`keyring`, which let malformed policy input normalize into an active secret
backend.

## Decision

Secret backend kind parsing rejects values with leading or trailing whitespace.
The parser remains case-insensitive for the canonical backend names to preserve
existing non-security-sensitive compatibility.

## Consequences

`file` and `keyring` remain valid backend kinds. Padded backend values such as
`" keyring "` now fail policy validation instead of normalizing into a secret
backend selection.
