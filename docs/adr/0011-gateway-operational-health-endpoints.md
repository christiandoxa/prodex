# ADR 0011: Gateway operational health endpoints

## Status

Accepted.

## Context

Production Kubernetes and load-balancer deployments need unauthenticated,
machine-readable health probes that do not consume provider quota and do not
require data-plane or admin credentials. The enterprise target explicitly calls
for `/livez`, `/readyz`, and `/startupz` plus readiness behavior that can reflect
local overload or draining.

## Decision

The runtime gateway serves public JSON probes before data-plane/admin routing:

- `GET /livez`
- `GET /readyz`
- `GET /startupz`

All probes return a stable `gateway.health` object with probe name, status,
readiness boolean, local-overload state, draining state, and active request
counters. `/readyz` returns `503` while local overload backoff is active or
while the gateway is draining; `/livez` and `/startupz` continue to report
process/startup liveness. Unsupported probe methods return `405` with an
`Allow: GET, HEAD` header.

## Consequences

- Kubernetes probes no longer need privileged gateway tokens.
- Local overload can remove a replica from readiness before upstream/provider
  traffic is attempted.
- Graceful shutdown uses the same response path to report `ready=false` during
  draining without failing liveness or startup probes.
