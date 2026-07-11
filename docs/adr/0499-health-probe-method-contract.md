# ADR 0499: Cover health probe method contracts uniformly

## Status

Accepted

## Context

Kubernetes and load balancers may call `/livez`, `/readyz`, or `/startupz` with
`GET` or `HEAD`. Unsupported methods must fail predictably without requiring
admin authentication.

## Decision

All gateway health probes expose the same public method contract:

- `GET` returns the JSON `gateway.health` body.
- `HEAD` returns the same status without a body.
- Unsupported methods return `405` with `Allow: GET, HEAD` and a small
  `gateway.health` body whose status is `method_not_allowed`.

The regression test
`gateway_operational_health_endpoints_are_public_and_machine_readable` now
covers this contract for all three probes.

## Consequences

Operational probe behavior stays consistent across orchestration systems while
keeping probe responses lightweight and unauthenticated.
