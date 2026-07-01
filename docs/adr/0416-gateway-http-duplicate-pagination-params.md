# 0416: Gateway HTTP duplicate pagination parameters

## Status

Accepted

## Context

Control-plane list and export routes use `limit` and `cursor` query parameters
for pagination. Duplicate parameters are ambiguous because adapters or
frameworks may choose first, last, or merged values.

## Decision

`prodex-gateway-http` now rejects duplicate `limit` and `cursor` query
parameters in `page_request_from_query`. Duplicate failures reuse the existing
redacted pagination error envelopes.

## Consequences

- Pagination semantics do not depend on framework query parsing order.
- Raw cursor and query values remain out of client-visible error messages.
