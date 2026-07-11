# ADR 0274: Application Quota Read Route Composition

## Status

Accepted

## Context

Gateway core now has a side-effect-free quota read authorization plan, but the
application boundary still only composed HTTP policy with inference admission.
That left quota read adapters without a shared route check before calling the
gateway authorization planner.

If adapters classify quota reads ad hoc, a control-plane route or inference
route can accidentally be wired to budget reads, weakening the requirement that
data-plane credentials cannot become quota bypasses and control-plane
credentials cannot read data-plane quota.

## Decision

Add `DataPlaneQuota` to the gateway HTTP route model for `/quota` and
`/v1/quota`. The route allows `GET`, requires trace context, and is separated
from inference, compact, websocket, control-plane, and health routes.

Add `plan_application_quota_read` to `prodex-application`. The planner composes
`plan_gateway_http_request` with
`plan_gateway_quota_read_authorization`, rejects any non-`DataPlaneQuota` route
before authorization, and returns stable redacted HTTP or authorization error
plans.

## Consequences

Composition roots can adapt quota read endpoints through one application use
case instead of reimplementing route and authorization checks. The application
crate remains independent of `prodex-authz`; it depends on the gateway-core
authorization plan rather than resolving authz boundaries directly.
