# ADR 0504: Allow stale-while-revalidate JWKS on the request path

## Status

Accepted

## Context

OIDC authentication must not fetch discovery or JWKS documents on the gateway
request path. At the same time, a still-usable stale JWKS cache should avoid
turning a transient IdP refresh delay into an authentication outage.

## Decision

Gateway request-path authentication may verify tokens with a stale-but-usable
JWKS cache and schedule refresh outside the request path. It still rejects
missing, empty, or expired-beyond-stale JWKS caches.

Regression coverage lives in
`authenticates_with_stale_while_revalidate_jwks_without_request_path_fetch`.

## Consequences

OIDC auth remains fail-closed for unusable key material while tolerating
short-lived IdP refresh delays without request-path network I/O.
