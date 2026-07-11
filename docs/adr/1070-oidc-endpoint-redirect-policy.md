# ADR 1070: OIDC Endpoint and Redirect Policy

## Status

Accepted

## Context

Gateway OIDC configuration used a standard URL parser at launch, but discovery
and JWKS refresh later passed strings to the shared HTTP client. Discovery could
replace the configured origin, redirects followed client defaults, DNS results
were not screened, and request authentication briefly spin-waited for startup
prefetch. These gaps made the identity-provider boundary an SSRF and availability
risk.

## Decision

Use one network-free `prodex-authn` endpoint policy for configured issuer and
JWKS values, the derived discovery endpoint, discovery-document `issuer` and
`jwks_uri`, and redirect behavior.

- `ValidatedOidcIssuer` and `ValidatedOidcEndpoint` use `url::Url` parsing and
  canonical serialization.
- URLs are bounded to 2048 bytes, require HTTPS and a normalized non-empty host,
  and reject whitespace, backslashes, userinfo, query parameters, fragments,
  port zero, trailing-dot hosts, and forbidden IP literals.
- Root issuer URLs normalize the implicit slash and default HTTPS port. Discovery
  metadata must contain an issuer with exactly the same normalized identity.
- JWKS uses the issuer origin by default. Cross-origin JWKS is possible only
  through an explicit full-origin allowlist; host and effective port both match.
- Redirects are disabled. A future manual redirect implementation must feed each
  target back through this policy before enabling a bounded hop count.

The compatibility refresh worker owns a dedicated no-proxy HTTPS client per
refresh. Its resolver rejects empty, excessive, private, loopback, link-local,
multicast, unspecified, documentation, benchmark, carrier-grade NAT, and
metadata-address results before connection. Resolver concurrency is one per
process; a timed-out blocking resolver retains that slot until it exits. The
connected peer must match the accepted resolution result, protecting against a
second DNS answer or connector substitution.

OIDC network work remains on one background refresh worker per gateway; DNS
resolution is additionally limited to one in-flight job per process. Request
authentication loads one immutable parsed `JwkSet` snapshot through `ArcSwap` and
fails closed; it performs no fetch, prefetch spin-wait, environment read, cache
mutex acquisition, or JSON reparse. The refresh worker validates the JWKS and
precomputes its fresh and last-known-good windows before atomically publishing
the snapshot. HTTP redirects are disabled, request/connect/resolve time is
bounded, documents are limited to 1 MiB, published JWKS sets to 128 keys, DNS
answers to 16, cache entries to 4, and timing/cache-control values are capped. Refresh-only cache map keys are
SHA-256 digests of canonical endpoint URLs, so URL material is not retained as a
key or emitted in diagnostics. Existing stale-while-revalidate, bounded
last-known-good, and failure-backoff behavior remains.

Insecure loopback OIDC endpoints exist only in `cfg(test)` compatibility
fixtures. Production constructors cannot create them.

## Consequences

- Malicious configuration, discovery metadata, DNS answers, redirects, and
  connected peers fail closed before their payload becomes trusted.
- A newly started gateway can reject OIDC authentication until background
  prefetch publishes a usable snapshot; request latency no longer depends on
  identity-provider latency.
- OIDC refresh no longer shares provider transport redirect, proxy, or DNS
  behavior.
- Cross-origin identity-provider deployments must explicitly allow each JWKS
  origin and port through `gateway.sso.oidc_jwks_origin_allowlist`.

## Verification

```bash
cargo test -p prodex-authn
cargo test -p prodex-runtime-policy
cargo test -p prodex-app gateway_oidc_ -- --test-threads=1
npm run ci:auth-boundary-guard
```
