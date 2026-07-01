# ADR 0326: Application Control-Plane If-Match Binding

## Status

Accepted.

## Context

Gateway HTTP can parse `If-Match` headers into domain `EntityTag` values, but
control-plane mutations still need application-level route/action validation
before a precondition is accepted. Without that binding, composition roots could
apply a parsed entity tag to the wrong control-plane operation or expose raw
request-controlled tag values through custom error handling.

## Decision

Add `plan_application_control_plane_precondition_from_http` and
`plan_application_control_plane_precondition_error_response` to
`prodex-application`.

The planner validates that the HTTP route maps to the same operation as the
authorized action request, delegates `If-Match` parsing to
`entity_tag_from_if_match_headers`, and exposes invalid tag or route/action
failures through stable redacted responses.

## Consequences

- Composition roots have one application boundary for optimistic-concurrency
  metadata before mutation planning.
- Route/action mismatches fail closed before storage compare-and-swap logic.
- Request-controlled `If-Match` values and lengths stay out of client-visible
  responses.
