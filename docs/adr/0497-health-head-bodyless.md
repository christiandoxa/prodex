# ADR 0497: Keep Health HEAD Probes Bodyless

## Status

Accepted.

## Context

Load balancers and orchestration systems may use `HEAD /readyz` for lightweight readiness checks. The response should preserve the same readiness status as `GET` without sending a JSON body.

## Decision

Health probes continue to accept `HEAD` and return the normal status code with an empty body. Regression tests cover `HEAD /readyz`, and OpenAPI documents `HEAD` probe responses without JSON content.

## Consequences

Operational probes remain cheap and predictable for orchestrators. JSON diagnostics stay available through `GET`.
