# ADR 0496: Stabilize Health Probe Method Denial Shape

## Status

Accepted.

## Context

Gateway health probes are unauthenticated operational endpoints. Unsupported methods should fail predictably without switching to an unrelated error envelope or leaking internals.

## Decision

Unsupported methods on `/livez`, `/readyz`, and `/startupz` keep returning `405` with `Allow: GET, HEAD` and a small `gateway.health` JSON body whose `status` is `method_not_allowed`. Regression tests cover the public shape, and OpenAPI documents the `405` response plus the `method_not_allowed` health status value.

## Consequences

Load balancers and diagnostics can parse method denials consistently. Runtime health behavior is unchanged.
