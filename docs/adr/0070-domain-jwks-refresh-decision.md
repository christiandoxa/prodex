# ADR 0070: Domain JWKS refresh decision

## Status

Accepted.

## Context

The enterprise OIDC target requires JWKS caching with refresh, bounded backoff,
stale-while-revalidate, last-known-good keys, and safe behavior when the IdP is
unavailable. `prodex-domain` already modeled issuer/audience/algorithm
validation and basic JWKS freshness, but it did not provide one shared decision
model for missing, fresh, stale, backoff, or unavailable key sets.

## Decision

Extend `JwksCacheSnapshot` with refresh-error and retry-after metadata, and add
`evaluate_jwks_refresh`. The decision order is: missing cache refreshes now,
fresh cache is used, retry backoff uses last-known-good keys when still within
the stale window, stale-but-usable cache triggers stale-while-revalidate, and
expired or empty cache is unavailable.

## Consequences

- Gateway and future authn/control-plane code can share consistent JWKS fallback
  semantics without performing network I/O in the domain crate.
- IdP outages can degrade safely to last-known-good only inside an explicit stale
  window.
- Metrics and diagnostics can derive key age, refresh failures, and unavailable
  state from this status model.
