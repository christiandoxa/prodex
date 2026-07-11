# ADR 0544: Normalize runtime policy cache roots

## Status

Accepted

## Context

Runtime policy cache entries were keyed by the root path exactly as passed by
the caller. Equivalent aliases such as `/var/lib/prodex` and
`/var/lib/prodex/.` could occupy separate cache entries, so an invalidation for
one spelling might leave the other spelling stale.

## Decision

Normalize runtime policy cache keys by removing `.` path components before
reading, storing, or invalidating entries. The invalidation plan reports the
normalized root and still exposes only whether an entry existed plus the cached
policy file version.

## Consequences

Publication-event consumers and reload paths invalidate equivalent root aliases
consistently. This is a lexical normalization only; it does not resolve
symlinks or require filesystem access.
