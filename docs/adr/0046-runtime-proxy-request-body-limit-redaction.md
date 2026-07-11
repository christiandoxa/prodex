# ADR 0046: Runtime proxy request body limit failures use generic responses and redacted diagnostics

## Status

Accepted

## Context

The standard runtime rotation proxy enforces a maximum HTTP request body size before forwarding traffic upstream. The rejection path previously returned and logged the raw body-limit error string, including byte-count details. While byte counts are less sensitive than payload content, the enterprise security posture requires local gateway/proxy errors to use stable non-leaking envelopes wherever possible.

The local rewrite gateway already uses metadata-only diagnostics for the same condition.

## Decision

When the standard runtime proxy rejects a request because the captured body exceeds the configured limit, it now:

- returns `413` with stable body `proxied request body is too large`;
- logs the stable event `request_body_too_large` with stable error metadata `request_body_too_large`;
- avoids logging the raw error string and byte-count details.

## Consequences

The request is still rejected before upstream routing, preserving admission semantics. Client-facing and runtime-log output are now stable and do not include request payloads, bearer tokens, or precise local byte-count internals.

## Validation

A regression test starts the standard runtime proxy with a small body limit, sends an oversized sensitive request, and verifies that:

- the proxy returns the generic `413` response;
- runtime diagnostics contain `request_body_too_large`;
- runtime diagnostics omit the bearer token, oversized request content, and raw byte-count phrase.
