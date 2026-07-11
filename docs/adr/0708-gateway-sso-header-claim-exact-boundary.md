# ADR 0708: Gateway SSO Header and Claim Exact Boundary

## Status

Accepted.

## Context

Gateway SSO header names and OIDC claim names control which identity attributes
become principals, roles, tenants, and key-prefix scopes. Runtime launch
previously trimmed these configured names before using them, and policy
validation only rejected blank-after-trim values.

## Decision

Treat `gateway.sso.*_header` and `gateway.sso.oidc_*_claim` names as exact
configuration boundaries. Policy validation rejects empty or whitespace-bearing
names, and runtime launch fails closed on invalid names instead of trimming or
defaulting them into active identity selectors.

## Consequences

Operators must correct padded SSO header and OIDC claim names in `policy.toml`.
The gateway no longer hides identity-boundary padding through runtime cleanup or
implicit fallback selectors.
