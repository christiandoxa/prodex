# ADR 0668: Gateway Admin If-Match Exact Boundary

## Status

Accepted.

## Context

Gateway admin key update and delete endpoints support `If-Match` for optimistic
concurrency. The enterprise HTTP/domain boundary validates entity tags exactly,
but the legacy gateway admin adapter trimmed `If-Match` values before comparing
them with the current ETag.

## Decision

The legacy gateway admin adapter now compares the exact `If-Match` header value.
Only the literal wildcard `*` bypasses ETag comparison, and only an exact current
ETag string satisfies a guarded mutation.

## Consequences

Padded wildcard or ETag values fail the existing redacted
`precondition_failed` path instead of being normalized into a successful
precondition. Existing canonical `If-Match` clients are unchanged.
