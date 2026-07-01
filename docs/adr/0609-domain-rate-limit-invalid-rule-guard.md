# ADR 0609: Domain Rate Limit Invalid Rule Guard

## Status

Accepted.

## Context

Redis rate-limit planning rejects zero limits and zero windows, but domain
rate-limit evaluation accepted those rules. A zero-window rule could admit a
request with a reset timestamp that does not advance.

## Decision

Domain rate-limit evaluation now rejects rules with `max_requests == 0` or
`window_seconds == 0`.

## Consequences

Admission and Redis planning share fail-closed semantics for invalid rule
configuration. Invalid rules must be corrected before requests can be admitted.
