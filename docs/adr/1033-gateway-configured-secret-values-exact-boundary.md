# ADR 1033: Gateway configured secret values exact boundary

## Status

Accepted.

## Context

Gateway policy references several bearer-token secrets through environment
variable names: admin tokens, static virtual keys, trusted SSO proxy tokens,
observability HTTP bearer tokens, and guardrail webhook bearer tokens. The
environment variable names were exact, but runtime resolution still trimmed the
secret values before hashing or forwarding them. That could make the active
credential differ from the deployed secret and hide malformed secret-manager
wiring.

## Decision

Configured gateway bearer secret values are exact runtime inputs. Referenced
environment variables must resolve to non-empty values without whitespace.
Runtime launch fails closed for empty or whitespace-bearing secret values.

## Consequences

Operators see malformed secret wiring at startup instead of silently using a
trim-normalized bearer token. Deployments should store canonical token values in
secret managers without padding or line breaks.
