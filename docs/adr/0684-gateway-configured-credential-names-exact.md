# ADR 0684: Gateway Configured Credential Names Match Exactly

## Status

Accepted.

## Context

Configured gateway admin token names and virtual key names identify
control-plane credentials and appear in audit, routing, and key management
surfaces. Policy validation only rejected trim-empty values, and runtime config
resolution trimmed names before putting them into active state.

## Decision

Configured gateway admin token names and virtual key names must now be exact
non-empty values without whitespace. Runtime config resolution no longer trims
these names; if settings are built directly, whitespace-bearing names are
rejected rather than normalized or mapped to empty credential identifiers.

## Consequences

Canonical credential names are unchanged. Padded or whitespace-bearing names
fail closed in policy files and no longer become active credential identifiers
through trim-normalization or direct runtime config fallback.
