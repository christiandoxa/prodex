# ADR 0323: Gateway HTTP If-Match Precondition

## Status

Accepted.

## Context

Control-plane mutation handlers need optimistic concurrency before updating
tenant-owned resources. The domain crate already owns `EntityTag`,
`require_matching_etag`, and stable redacted entity-tag error envelopes. Gateway
HTTP adapters need a framework-neutral way to extract the `If-Match` header so
composition roots can pass a validated precondition into application and
control-plane planning.

## Decision

Add `entity_tag_from_if_match_headers` and
`plan_gateway_http_entity_tag_error_response` to `prodex-gateway-http`.

The extractor treats `If-Match` as optional at the HTTP boundary, validates a
present value with the domain `EntityTag` parser, and leaves the policy decision
about whether a mutation requires a precondition to the control-plane or
application layer. Invalid tags are mapped through the domain redacted response
plan so request-controlled tag values and lengths stay out of client-visible
messages.

## Consequences

- Concrete HTTP adapters can validate optimistic-concurrency metadata without
  depending on Axum, Hyper, Tower, or storage crates.
- Control-plane mutation planners can require a parsed entity tag when a
  resource action needs compare-and-swap semantics.
- Error compatibility stays centralized in the domain API governance envelope.
