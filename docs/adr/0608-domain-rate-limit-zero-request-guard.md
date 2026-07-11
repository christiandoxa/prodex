# ADR 0608: Domain Rate Limit Zero Request Guard

## Status

Accepted.

## Context

Rate-limit atomic update planning rejects zero increments, but rate-limit
evaluation could still return `Allow` for a zero-request admission. That created
a split contract between admission evaluation and the later mutation plan.

## Decision

Rate-limit evaluation now rejects requests with `requested_requests == 0`.

## Consequences

Admission and mutation planning share the same fail-closed zero-request
semantics. Callers must request at least one unit before a bucket can be
admitted or mutated.
