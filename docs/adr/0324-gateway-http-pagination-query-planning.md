# ADR 0324: Gateway HTTP Pagination Query Planning

## Status

Accepted.

## Context

Control-plane list and audit/export APIs need stable pagination semantics before
concrete HTTP adapters dispatch to application services. The domain crate owns
`Cursor`, `PageRequest`, clamped page limits, and redacted cursor error
responses. Gateway HTTP adapters need a framework-neutral query parser that can
validate request-controlled cursor values without depending on Axum, Hyper, or a
URL parsing crate in the boundary layer.

## Decision

Add `page_request_from_query` and
`plan_gateway_http_pagination_query_error_response` to `prodex-gateway-http`.

The planner reads `limit` and `cursor` query parameters, validates cursors with
the domain `Cursor` parser, and delegates final limit clamping/defaulting to
`PageRequest::new`. Invalid cursors reuse the domain redacted cursor envelope;
invalid numeric limits use a stable gateway error that does not expose the
request-controlled value.

## Consequences

- HTTP adapters can normalize pagination requests before calling control-plane
  application code.
- Cursor length and encoding details remain hidden from client-visible
  responses.
- Pagination semantics remain domain-owned while gateway HTTP handles only
  query-shape extraction.
