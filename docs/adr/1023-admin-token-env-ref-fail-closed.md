# ADR 1023: Admin token environment references fail closed

## Status

Accepted.

## Context

Configured gateway admin tokens are control-plane credentials loaded from
environment variables named by `token_env`. Runtime policy validation rejects
malformed env-var names, but direct launch configuration still ignored invalid,
missing, or empty env references. That could silently drop an intended admin
credential and leave operators with a different control-plane auth surface than
the configured policy described.

## Decision

Admin token launch configuration now fails closed when `token_env` is empty,
contains whitespace, is missing from the process environment, or resolves to an
empty secret. Valid env-var names and non-empty secrets continue to work
unchanged.

## Consequences

Misconfigured admin-token secrets are visible at startup instead of being
silently skipped. Deployments that intentionally disable an additional admin
token should remove the `[[gateway.admin_tokens]]` entry rather than leaving an
unset or empty `token_env` reference.
