# 0628: Domain OIDC Issuer Host Shape Guard

## Status

Accepted

## Context

OIDC issuer validation already required `https://` and a non-empty host before
discovery URLs could be derived. The host check still accepted ambiguous values
with whitespace or userinfo, such as `https://user:pass@idp.example.com`.

## Decision

`Issuer::new` now validates the extracted host with a small ASCII allowlist. It
rejects empty hosts, userinfo, whitespace, and other non-transport-safe
characters while preserving ordinary DNS names, ports, and bracketed IPv6
literals.

## Consequences

- Background OIDC discovery cannot derive targets from ambiguous issuer hosts.
- Request-path authentication keeps using already validated issuer policy.
- No URL parser dependency is added to the domain crate.
