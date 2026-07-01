# ADR 0679: Gateway Admin Key Prefix Scopes Match Exactly

## Status

Accepted.

## Context

Configured gateway admin tokens can be scoped with `allowed_key_prefixes`.
Those prefixes are authorization scope inputs. Policy validation only rejected
trim-empty values, and runtime config resolution trimmed prefixes before putting
them into active state.

## Decision

Configured gateway admin key-prefix scopes must now be exact non-empty values
without whitespace. Runtime config resolution no longer trims prefix scopes; if
a caller builds policy settings directly, whitespace-bearing prefixes are
ignored rather than normalized.

## Consequences

Canonical prefix scopes are unchanged. Padded or whitespace-bearing scope
strings no longer become active admin authorization scopes.
