# ADR 0569: Redact runtime launch quota preflight warnings

## Status

Accepted

## Context

Runtime launch quota preflight can fail open when a selected profile's quota
check cannot be fetched. The terminal warning printed the detailed `{err:#}`
chain directly. That chain can include bearer tokens, key-bearing URLs, proxy
diagnostics, or provider response details.

## Decision

Keep the existing warning and fail-open launch behavior, but redact the detailed
preflight error chain before rendering it in the terminal notice.

## Consequences

- Runtime launch still continues without the quota gate when selected-profile
  preflight fails.
- Operators keep non-sensitive context about the failed preflight.
- Secret-like values from quota/proxy/provider diagnostics are removed from the
  terminal notice.
