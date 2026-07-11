# ADR 0081: Authn boundary crate

## Status
Accepted

## Context
Enterprise authentication must avoid OIDC discovery and JWKS network fetches on
the request path. The domain crate already models issuer, audience, JWT
algorithm policy, JWKS cache freshness, role mapping, principals, and tenant
context. Gateway and control-plane handlers need a reusable authentication
boundary that converts already-decoded and signature-checked token metadata into
canonical `Principal` values without importing HTTP, storage, or network code.

## Decision
Introduce `prodex-authn` as a network-free authentication boundary crate. It
validates token metadata against an `OidcValidationPolicy`, a supplied
`JwksCacheSnapshot`, an explicit role mapper, token time bounds, known key ID,
signature verification status, and mandatory tenant claim.

The crate rejects request-path authentication when no usable JWKS snapshot is
available. Fresh, stale-while-revalidate, and last-known-good-during-backoff
snapshots are accepted; missing snapshots and unusable stale snapshots are not.
Missing or unknown role claims remain denied rather than defaulting to Admin.

## Consequences
Serving adapters can perform discovery and JWKS refresh in bounded background
control-plane/cache code, then pass immutable snapshots into request handlers.
This keeps request authentication deterministic, testable, tenant-aware, and
free from network side effects.
