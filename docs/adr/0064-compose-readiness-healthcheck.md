# ADR 0064: Compose readiness healthcheck

## Status

Accepted.

## Context

The gateway exposes public operational probes (`/livez`, `/readyz`, and
`/startupz`) for container orchestration. The compose healthcheck previously
called an authenticated admin OpenAPI endpoint, which required interpolating the
gateway bearer token into a shell command. That leaks more privilege than needed
for readiness and can expose the token through container process inspection.

## Decision

Use the public `/readyz` endpoint for the compose healthcheck and add a CI guard
that rejects bearer-token healthchecks. Keep admin OpenAPI access documented as
an operator action, not as the orchestration readiness probe.

## Consequences

- Container readiness no longer needs admin credentials.
- The compose healthcheck aligns with Kubernetes readiness semantics.
- Admin endpoint compatibility is unchanged.
