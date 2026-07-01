# ADR 0325: Application Control-Plane Pagination Query Binding

## Status

Accepted.

## Context

Gateway HTTP now provides a framework-neutral pagination query parser, but
control-plane use cases still need application-level route/action validation
before a parsed page request is accepted for billing, audit, or other list/query
operations. Without that binding, composition roots could parse `limit` and
`cursor` independently or apply pagination metadata to the wrong operation.

## Decision

Add `plan_application_control_plane_page_request_from_http_query` and
`plan_application_control_plane_page_request_error_response` to
`prodex-application`.

The planner validates that the HTTP route maps to the same control-plane
operation as the authorized action request, delegates query parsing to
`page_request_from_query`, and exposes route/action or pagination failures
through stable redacted responses.

## Consequences

- Composition roots have one side-effect-free application boundary for
  control-plane pagination metadata.
- Route/action mismatches fail closed before storage query planning.
- Cursor and limit parse details remain out of client-visible responses.
