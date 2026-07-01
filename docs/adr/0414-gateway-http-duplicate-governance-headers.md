# 0414: Gateway HTTP duplicate governance headers

## Status

Accepted

## Context

`Idempotency-Key` and `If-Match` are request-controlled governance headers for
mutating control-plane operations. If a request carries duplicate values,
different HTTP adapters, proxies, or framework extractors may choose different
headers, which can break replay safety or optimistic concurrency.

## Decision

`prodex-gateway-http` now rejects duplicate `Idempotency-Key` and `If-Match`
headers in the shared header parsers. Duplicate failures reuse the existing
redacted invalid idempotency-key and entity-tag response envelopes.

## Consequences

- Ambiguous replay/precondition headers fail closed before application planning.
- Header values remain out of client-visible error messages.
- Legacy gateway admin composition now delegates `Idempotency-Key` and
  `If-Match` parsing to the same shared helpers instead of choosing one
  duplicate header ad hoc.
